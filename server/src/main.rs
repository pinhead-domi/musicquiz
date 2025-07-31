use core::num;
use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc, Mutex};
use std::{thread, usize};

use ratatui::widgets::List;
use serde::{Deserialize, Serialize};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

use log::LevelFilter;

trait LogExt {
    fn log(self) -> Self;
}

impl<T, E> LogExt for Result<T, E>
where
    E: std::fmt::Display,
{
    fn log(self) -> Self {
        if let Err(e) = &self {
            log::error!("{}", e)
        }
        self
    }
}

enum Command {
    Transfer,
    Play,
    Pause,
    Repeat,
    Reveal,
}

enum AppEvent {
    CrossTerm(crossterm::event::Event),
    ClientUpdate,
}

#[derive(Deserialize, Debug, Clone)]
struct TitleInfo {
    title: String,
    interpret: String,
}

impl Widget for TitleInfo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(vec![
            Line::from(vec!["Title: ".blue().bold(), self.title.as_str().into()]),
            Line::from(vec![
                "Interpret: ".yellow().bold(),
                self.interpret.as_str().into(),
            ]),
        ])
        .block(title_block("Current Title"))
        .gray()
        .render(area, buf);
    }
}

fn title_block(title: &str) -> Block {
    Block::bordered()
        .gray()
        .title(title.bold().into_centered_line())
}

#[derive(Deserialize, Debug)]
struct TitleList {
    titles: Vec<TitleInfo>,
}

struct ConnectionInfo {
    active_clients: u8,
    transfered: bool,
    playing: bool,
}

impl Widget for ConnectionInfo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(vec![
            Line::from(vec![
                "Number of clients: ".into(),
                self.active_clients.to_string().yellow().bold(),
            ]),
            Line::from(vec![
                "Transferred: ".into(),
                self.transfered.to_string().yellow().bold(),
            ]),
            Line::from(vec![
                "Playing: ".into(),
                self.playing.to_string().yellow().bold(),
            ]),
        ])
        .block(title_block("Connection Info"))
        .gray()
        .render(area, buf);
    }
}

struct GameInfo {
    titles_correct: u8,
    interprets_correct: u8,
    current_index: u8,
    total_num: u8,
}

impl Widget for GameInfo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let incorrect_titles: u8 = self.current_index - self.titles_correct;
        let incorrect_interprets: u8 = self.current_index - self.interprets_correct;

        Paragraph::new(vec![
            Line::from(vec![
                "Titles: ".into(),
                self.titles_correct.to_string().green().bold(),
                " + ".into(),
                incorrect_titles.to_string().red().bold(),
                " / ".into(),
                self.total_num.to_string().into(),
            ]),
            Line::from(vec![
                "Interprets: ".into(),
                self.interprets_correct.to_string().green().bold(),
                " + ".into(),
                incorrect_interprets.to_string().red().bold(),
                " / ".into(),
                self.total_num.to_string().into(),
            ]),
        ])
        .block(title_block("Game Info"))
        .gray()
        .render(area, buf);
    }
}

#[derive(Debug, Clone)]
struct Grading {
    interpret: Option<bool>,
    title: Option<bool>,
}

#[derive(Debug, Clone)]
struct SongInfo {
    title: TitleInfo,
    next: Option<TitleInfo>,
    grading: Grading,
}

#[derive(Debug)]
struct Client {
    stream: TcpStream,
    nickname: String,
}

impl Widget for SongInfo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title_grading = match self.grading.title {
            None => "not yet graded".gray().bold(),
            Some(grade) => match grade {
                true => "correct".green().bold(),
                false => "incorrect".red().bold(),
            },
        };

        let interpret_grading = match self.grading.interpret {
            None => "not yet graded".gray().bold(),
            Some(grade) => match grade {
                true => "correct".green().bold(),
                false => "incorrect".red().bold(),
            },
        };

        let mut line_elements = vec![
            Line::from(vec![
                "Title: ".blue().bold(),
                self.title.title.as_str().into(),
                " - ".into(),
                title_grading,
            ]),
            Line::from(vec![
                "Interpret: ".yellow().bold(),
                self.title.interpret.as_str().into(),
                " - ".into(),
                interpret_grading,
            ]),
        ];

        if let Some(next) = self.next {
            line_elements.push(Line::from(vec![]));
            line_elements.push(Line::from(vec!["Coming up: ".gray().bold()]));

            let append_title = vec!["Interpret: ".blue().bold(), next.title.to_string().into()];
            line_elements.push(append_title.into());

            let append_interpret = vec![
                "Interpret: ".yellow().bold(),
                next.interpret.to_string().into(),
            ];
            line_elements.push(append_interpret.into());
        }

        Paragraph::new(line_elements)
            .block(title_block("Current Title"))
            .gray()
            .render(area, buf);
    }
}

#[derive(Debug)]
struct App {
    title: u32,
    exit: bool,
    playing: bool,
    transfered: bool,
    handles: Arc<Mutex<Vec<Client>>>,
    event_channel: Receiver<AppEvent>,
    titles: TitleList,
    current_grading: Grading,
    grading_history: Vec<Grading>,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), Box<dyn Error>> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?
        }
        Ok(())
    }
    fn draw(&self, frame: &mut Frame) {
        let outer_layout =
            Layout::vertical(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(frame.area());

        let inner_layout = Layout::horizontal(vec![
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Fill(1),
        ])
        .split(outer_layout[1]);

        let connection_info = ConnectionInfo {
            active_clients: self.handles.lock().unwrap().len() as u8,
            transfered: self.transfered,
            playing: self.playing,
        };

        let titles_correct = self
            .grading_history
            .iter()
            .filter(|grad| grad.title.is_some_and(|val| val))
            .count() as u8;

        let interprets_correct = self
            .grading_history
            .iter()
            .filter(|grad| grad.interpret.is_some_and(|val| val))
            .count() as u8;

        let game_info = GameInfo {
            titles_correct,
            interprets_correct,
            current_index: self.title as u8,
            total_num: self.titles.titles.len() as u8,
        };

        let next = if (self.title as usize) < self.titles.titles.len() - 1 {
            Some(self.titles.titles[self.title as usize + 1].clone())
        } else {
            None
        };

        let song_info = SongInfo {
            title: self.titles.titles[self.title as usize].clone(),
            next,
            grading: self.current_grading.clone(),
        };

        frame.render_widget(song_info, outer_layout[0]);
        frame.render_widget(connection_info, inner_layout[0]);
        frame.render_widget(game_info, inner_layout[1]);

        let nicknames: Vec<String> = self
            .handles
            .lock()
            .log()
            .unwrap()
            .iter()
            .map(|client| client.nickname.clone())
            .collect();

        List::new(nicknames)
            .block(title_block("Clients"))
            .render(inner_layout[2], frame.buffer_mut());
    }
    fn handle_events(&mut self) -> Result<(), Box<dyn Error>> {
        match self.event_channel.recv()? {
            AppEvent::ClientUpdate => {}
            AppEvent::CrossTerm(event) => match event {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.match_key_event(key_event)?;
                }
                _ => {}
            },
        }
        Ok(())
    }
    fn match_key_event(&mut self, event: KeyEvent) -> Result<(), Box<dyn Error>> {
        match event.code {
            KeyCode::Char('o') => {
                self.pause();
            }
            KeyCode::Char('p') => {
                self.play();
            }
            KeyCode::Char('t') => {
                if !self.transfered {
                    self.transfered = true;
                    self.transfer_file()?;
                }
            }
            KeyCode::Char('a') => {
                self.grade_title(false);
            }
            KeyCode::Char('s') => {
                self.grade_title(true);
            }
            KeyCode::Char('y') => {
                self.grade_interpret(false);
            }
            KeyCode::Char('x') => {
                self.grade_interpret(true);
            }
            KeyCode::Char('n') => {
                self.next()?;
            }
            KeyCode::Char('r') => {
                self.repeat();
            }
            KeyCode::Char('q') => {
                self.exit = true;
            }
            _ => {}
        }
        Ok(())
    }
    fn play(&mut self) {
        if !self.playing && self.transfered {
            self.playing = true;
            match self.send_command(Command::Play) {
                Ok(_) => {}
                Err(_) => {
                    self.exit = true;
                }
            }
        }
    }
    fn next(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(Command::Pause)?;
        self.playing = false;

        if self.current_grading.title.is_some() && self.current_grading.interpret.is_some() {
            self.grading_history.push(self.current_grading.clone());

            let title = &self.titles.titles[self.title as usize];
            log::info!(
                "Grading for song {} - {}: Title: {}, Interpret: {}",
                title.title,
                title.interpret,
                self.current_grading.title.unwrap(),
                self.current_grading.interpret.unwrap()
            );

            self.send_command(Command::Reveal)?;

            self.reset_grading();
            if (self.title as usize) < self.titles.titles.len() - 1 {
                self.transfered = false;
                self.title += 1;
            }
        }

        Ok(())
    }
    fn repeat(&mut self) {
        if self.transfered {
            self.playing = false;
            match self.send_command(Command::Repeat) {
                Ok(_) => {}
                Err(_) => {
                    self.exit = true;
                }
            }
        }
    }
    fn pause(&mut self) {
        if self.playing && self.transfered {
            self.playing = false;
            match self.send_command(Command::Pause) {
                Ok(_) => {}
                Err(_) => {
                    self.exit = true;
                }
            }
        }
    }
    fn reset_grading(&mut self) {
        self.current_grading = Grading {
            title: None,
            interpret: None,
        }
    }
    fn grade_title(&mut self, grade: bool) {
        self.current_grading.title = Some(grade);
    }
    fn grade_interpret(&mut self, grade: bool) {
        self.current_grading.interpret = Some(grade);
    }
    fn transfer_file(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(Command::Transfer)?;
        Ok(())
    }
    fn send_command(&mut self, command: Command) -> Result<(), Box<dyn Error>> {
        let numeric: u8 = match command {
            Command::Play => 1,
            Command::Transfer => 2,
            Command::Pause => 3,
            Command::Repeat => 4,
            Command::Reveal => 5,
        };

        let bytes = numeric.to_be_bytes();

        self.handles.lock().log().unwrap().retain_mut(|client| {
            let mut keep = true;
            keep &= client.stream.write_all(&bytes).is_ok();
            if keep && numeric == 2 {
                keep &= stream_file(
                    &mut client.stream,
                    format!(
                        "C:/Users/Dominik Haring/Documents/GitHub/musicquiz/{}.mp3",
                        self.title + 1
                    )
                    .as_str(),
                )
                .is_ok();
            } else if keep && numeric == 5 {
                keep &= stream_title_grading(
                    &mut client.stream,
                    self.current_grading.clone(),
                    self.titles.titles[self.title as usize].clone(),
                )
                .is_ok();
            }
            if !keep {
                log::info!("Client {:?} will be dropped", client);
            }

            keep
        });

        Ok(())
    }
}

fn read_nickname(stream: &mut TcpStream) -> Result<String, Box<dyn Error>> {
    let mut bytes_to_read = [0_u8; 64 / 8];
    stream.read_exact(&mut bytes_to_read)?;

    let length_numeric = u64::from_be_bytes(bytes_to_read);
    let mut buffer = vec![0_u8; length_numeric as usize];

    stream.read_exact(&mut buffer)?;

    let nickname = String::from_utf8(buffer)?;
    Ok(nickname)
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logging::log_to_file("test.log", LevelFilter::Info)?;

    let file_content = match fs::read_to_string(
        "C:/Users/Dominik Haring/Documents/GitHub/musicquiz/titles.json",
    ) {
        Ok(content) => content,
        Err(e) => {
            log::error!("Could not open title list file: [{}]", e.to_string());
            return Err(e.into());
        }
    };
    let titles: TitleList = match serde_json::from_str(&file_content) {
        Ok(titles) => titles,
        Err(e) => {
            log::error!("Could not parse the title list file: [{}]", e.to_string());
            return Err(e.into());
        }
    };

    log::info!("Title list loaded and parsed successfully!");

    let mut terminal = ratatui::init();
    let listener = match TcpListener::bind("0.0.0.0:59683") {
        Ok(listener) => listener,
        Err(e) => {
            log::error!("Could not open the given TCP port: [{}]", e.to_string());
            return Err(e.into());
        }
    };

    let (tx, rx) = mpsc::channel::<AppEvent>();
    let clients = Arc::new(Mutex::new(Vec::<Client>::new()));
    let acceptor = clients.clone();

    let t1 = tx.clone();
    let t2 = tx.clone();

    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            if let Ok(addr) = stream.peer_addr() {
                log::info!("New client connected at {}", addr);
            }

            if let Ok(nickname) = read_nickname(&mut stream) {
                log::info!("Client connected with nickname {}", &nickname);

                let client = Client { nickname, stream };
                acceptor.lock().log().unwrap().push(client);
                if !t1.send(AppEvent::ClientUpdate).is_ok() {
                    log::error!("Unable to send AppEvents to worker thread, no new connections will we accepted!");
                    break;
                }
            }
        }
    });

    thread::spawn(move || loop {
        match event::read() {
            Ok(event) => {
                if let Err(e) = t2.send(AppEvent::CrossTerm(event)) {
                    log::error!("{}: keystrokes will no longer be forwarded", e);
                    break;
                }
            }
            Err(e) => {
                log::error!("{}: keystrokes will no longer be forwarded", e);
                break;
            }
        }
    });

    let app_result = App {
        title: 0,
        playing: false,
        transfered: false,
        exit: false,
        handles: clients,
        event_channel: rx,
        titles,
        current_grading: Grading {
            title: None,
            interpret: None,
        },
        grading_history: Vec::new(),
    }
    .run(&mut terminal)
    .log();

    ratatui::restore();
    return app_result;
}

fn stream_file(stream: &mut TcpStream, path: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();

    let mut bytes: Vec<u8> = vec![0; file_size as usize];
    file.read_exact(&mut bytes)?;

    let size_as_bytes = file_size.to_be_bytes();

    stream.write_all(&size_as_bytes)?;
    stream.write_all(&bytes)?;

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct TransferTitleGrading {
    title: String,
    interpret: String,
    title_grading: bool,
    interpret_grading: bool,
}

fn stream_title_grading(
    stream: &mut TcpStream,
    grading: Grading,
    title: TitleInfo,
) -> Result<(), Box<dyn Error>> {
    let transfer_item = TransferTitleGrading {
        title: title.title,
        interpret: title.interpret,
        title_grading: grading.title.unwrap_or(false),
        interpret_grading: grading.interpret.unwrap_or(false),
    };
    let transfer_string = serde_json::to_string(&transfer_item)?;
    let transfer_data = transfer_string.as_bytes();
    let transfer_size = transfer_data.len().to_be_bytes();

    log::info!("Transfering {} to {:?}", transfer_string, stream);

    stream.write_all(&transfer_size)?;
    stream.write_all(&transfer_data)?;

    Ok(())
}

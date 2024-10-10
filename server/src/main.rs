use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use serde::Deserialize;

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

enum Command {
    Transfer,
    Play,
    Pause,
    Repeat,
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
    total_num: u8,
    current_index: u8,
}

impl Widget for GameInfo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let incorrect_titles: u8 = 0;
        let incorrect_interprets: u8 = 0;

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
    grading: Grading,
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

        Paragraph::new(vec![
            Line::from(vec![
                "Title: ".blue().bold(),
                self.title.title.as_str().into(),
                " - ".into(),
                title_grading
            ]),
            Line::from(vec![
                "Interpret: ".yellow().bold(),
                self.title.interpret.as_str().into(),
                " - ".into(),
                interpret_grading
            ]),
        ])
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
    handles: Arc<Mutex<Vec<TcpStream>>>,
    event_channel: Receiver<AppEvent>,
    titles: TitleList,
    current_grading: Grading,
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

        let inner_layout =
            Layout::horizontal(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(outer_layout[1]);

        let connection_info = ConnectionInfo {
            active_clients: self.handles.lock().unwrap().len() as u8,
            transfered: self.transfered,
            playing: self.playing,
        };

        let game_info = GameInfo {
            titles_correct: 0,
            interprets_correct: 0,
            current_index: self.title as u8,
            total_num: self.titles.titles.len() as u8,
        };

        let song_info = SongInfo{
            title: self.titles.titles[self.title as usize].clone(),
            grading: self.current_grading.clone()
        };

        frame.render_widget(song_info, outer_layout[0]);
        frame.render_widget(connection_info, inner_layout[0]);
        frame.render_widget(game_info, inner_layout[1]);
    }
    fn handle_events(&mut self) -> Result<(), Box<dyn Error>> {
        match self.event_channel.recv()? {
            AppEvent::ClientUpdate => {}
            AppEvent::CrossTerm(event) => match event {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.match_key_event(key_event);
                }
                _ => {}
            },
        }
        Ok(())
    }
    fn match_key_event(&mut self, event: KeyEvent) {
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
                    self.transfer_file();
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
                self.next().unwrap();
            }
            KeyCode::Char('r') => {
                self.repeat();
            }
            KeyCode::Char('q') => {
                self.exit = true;
            }
            _ => {}
        }
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
        if (self.title as usize) < self.titles.titles.len() - 1 {
            self.send_command(Command::Pause)?;
            self.playing = false;

            self.reset_grading();

            self.transfered = false;
            self.title += 1;
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
    fn transfer_file(&mut self) {
        self.send_command(Command::Transfer).unwrap();
    }
    fn send_command(&mut self, command: Command) -> Result<(), Box<dyn Error>> {
        let numeric: u8 = match command {
            Command::Play => 1,
            Command::Transfer => 2,
            Command::Pause => 3,
            Command::Repeat => 4,
        };

        let bytes = numeric.to_be_bytes();

        self.handles.lock().unwrap().retain_mut(|client| {
            let mut keep = true;
            keep &= client.write_all(&bytes).is_ok();
            if keep && numeric == 2 {
                keep &= stream_file(
                    client,
                    format!("C:/Users/Dominik Haring/Documents/music quiz/{}.mp3", self.title + 1).as_str(),
                )
                .is_ok();
            }

            keep
        });

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let file_content = fs::read_to_string("C:/Users/Dominik Haring/Documents/GitHub/musicquiz/titles.json")?;
    let titles: TitleList = serde_json::from_str(&file_content)?;

    let mut terminal = ratatui::init();
    let listener = TcpListener::bind("0.0.0.0:6969")?;

    let (tx, rx) = mpsc::channel::<AppEvent>();
    let clients = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
    let acceptor = clients.clone();

    let t1 = tx.clone();
    let t2 = tx.clone();

    thread::spawn(move || {
        for client in listener.incoming().flatten() {
            acceptor.lock().unwrap().push(client);
            t1.send(AppEvent::ClientUpdate).unwrap();
        }
    });

    thread::spawn(move || loop {
        let event = event::read().unwrap();
        t2.send(AppEvent::CrossTerm(event)).unwrap();
    });

    let _app_result = App {
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
    }
    .run(&mut terminal);

    ratatui::restore();
    Ok(())
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

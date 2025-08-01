use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Widget};
use ratatui::{DefaultTerminal, Frame};
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
use std::{
    error::Error,
    io::{self, Cursor, ErrorKind, Read, Write},
    net::TcpStream,
};

#[derive(Clone)]
enum Command {
    Play,
    Transfer,
    Pause,
    Repeat,
    Reveal,
}

enum AppEvent {
    Command(Command),
    SongData(Vec<u8>),
    TitleGrading(TitleGrading),
    CrossTerm(crossterm::event::Event),
    Disconnected,
}

enum AppState {
    EnterNickname,
    Disconnected,
    Paused,
    Playing,
    Revealing,
}

impl Display for AppState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            AppState::EnterNickname => "NICKNAME CONFIG",
            AppState::Disconnected => "DISCONNECTED",
            AppState::Paused => "PAUSED",
            AppState::Playing => "PLAYING",
            AppState::Revealing => "REVEALING",
        };
        f.write_str(display)?;
        Ok(())
    }
}
struct App {
    connection_string: String,
    nickname: String,
    state: AppState,
    event_loop: Receiver<AppEvent>,
    stream: Option<thread::JoinHandle<()>>,
    event_sender: Sender<AppEvent>,
    current_song: Option<Vec<u8>>,
    current_title_grading: Option<TitleGrading>,
    sink: Sink,
    volume: f32,
    exit: bool,
}

struct NickNamePopup {
    nickname: String,
}

impl Widget for NickNamePopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(" Enter Nickname ");
        Paragraph::new(vec![Line::from(vec![self.nickname.as_str().gray().bold()])])
            .block(block)
            .gray()
            .render(area, buf);
    }
}

struct ServerPopup {
    url: String,
}

impl Widget for ServerPopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(" Enter Server Url ");
        Paragraph::new(vec![Line::from(vec![self.url.as_str().gray().bold()])])
            .block(block)
            .gray()
            .render(area, buf);
    }
}

impl Widget for TitleGrading {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(" Result from previous title ");

        let title = self.title.as_str().gray().bold();
        let title_guess = if self.title_grading {
            "correct".green().bold()
        } else {
            "incorrect".red().bold()
        };

        let interpret = self.interpret.as_str().gray().bold();
        let interpret_guess = if self.interpret_grading {
            "correct".green().bold()
        } else {
            "incorrect".red().bold()
        };

        Paragraph::new(vec![
            Line::from(vec!["Title: ".blue(), title, " - ".into(), title_guess]),
            Line::from(vec![
                "Interpret: ".yellow(),
                interpret,
                " - ".into(),
                interpret_guess,
            ]),
        ])
        .block(block)
        .gray()
        .render(area, buf);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TitleGrading {
    title: String,
    interpret: String,
    title_grading: bool,
    interpret_grading: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), Box<dyn Error>> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?
        }
        Ok(())
    }
    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let layout =
            Layout::vertical(vec![Constraint::Percentage(80), Constraint::Fill(1)]).split(area);

        let show_popup = matches!(
            self.state,
            AppState::EnterNickname | AppState::Disconnected | AppState::Revealing
        );

        let block = Block::bordered().title(" Music Quiz Client ");
        Paragraph::new(vec![
            Line::from(vec![
                "Nickname: ".into(),
                self.nickname.clone().yellow().bold(),
            ]),
            Line::from(vec![
                "Server url: ".into(),
                self.connection_string.clone().blue().bold(),
            ]),
            Line::from(vec![
                "Status: ".into(),
                format!("{}", &self.state).green().bold(),
            ]),
        ])
        .block(block)
        .render(layout[0], frame.buffer_mut());

        let audio_block = Block::bordered().title(" Audio Level ");
        Gauge::default()
            .block(audio_block)
            .percent((self.volume * 100.0) as u16)
            .render(layout[1], frame.buffer_mut());

        if show_popup {
            let area = popup_area(area, 60, 20);
            frame.render_widget(Clear, area); //this clears out the background

            match self.state {
                AppState::EnterNickname => {
                    frame.render_widget(
                        NickNamePopup {
                            nickname: self.nickname.clone(),
                        },
                        area,
                    );
                }
                AppState::Disconnected => {
                    frame.render_widget(
                        ServerPopup {
                            url: self.connection_string.clone(),
                        },
                        area,
                    );
                }
                AppState::Revealing => {
                    frame.render_widget(self.current_title_grading.clone().unwrap(), area);
                }
                _ => {}
            }
        }
    }
    fn handle_events(&mut self) -> Result<(), Box<dyn Error>> {
        match self.event_loop.recv()? {
            AppEvent::Command(cmd) => {
                match cmd {
                    Command::Play => self.play(),
                    Command::Transfer => { /*Should not happen TM*/ }
                    Command::Pause => self.pause(),
                    Command::Repeat => {
                        if let Some(song) = self.current_song.clone() {
                            self.append_song(song)?;
                        }
                    }
                    Command::Reveal => { /*Should also not happen TM*/ }
                }
            }
            AppEvent::SongData(song) => {
                self.current_song = Some(song.clone());
                self.append_song(song)?;
            }
            AppEvent::TitleGrading(title_grading) => {
                self.current_title_grading = Some(title_grading);
                self.state = AppState::Revealing;
            }
            AppEvent::CrossTerm(event) => match event {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    match self.state {
                        AppState::EnterNickname => {
                            self.handle_nickname_input(key_event);
                        }
                        AppState::Disconnected => {
                            self.handle_url_input(key_event);
                        }
                        _ => {
                            self.handle_input(key_event);
                        }
                    }
                }
                _ => {}
            },
            AppEvent::Disconnected => {
                self.stream.take().map(|stream| stream.join());
                self.clear();
                self.state = AppState::Disconnected;
            }
        }

        Ok(())
    }

    fn handle_nickname_input(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char(new) => {
                self.nickname.push(new);
            }
            KeyCode::Backspace => {
                self.nickname.pop();
            }
            KeyCode::Enter => {
                self.state = AppState::Disconnected;
            }
            KeyCode::Esc => {
                self.exit = true;
            }
            _ => {}
        }
    }

    fn handle_url_input(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char(new) => {
                self.connection_string.push(new);
            }
            KeyCode::Backspace => {
                self.connection_string.pop();
            }
            KeyCode::Enter => {
                self.connect();
            }
            KeyCode::Esc => {
                self.exit = true;
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char('q') => {
                self.exit = true;
            }
            KeyCode::Char('+') => {
                self.increase_volume();
            }
            KeyCode::Char('-') => {
                self.decrease_volume();
            }
            _ => {}
        }
    }
    fn connect(&mut self) {
        if let Ok(mut stream) = TcpStream::connect(self.connection_string.as_str()) {
            self.state = AppState::Paused;
            let sender = self.event_sender.clone();
            let err_sender = self.event_sender.clone();

            self.send_nickname(&mut stream);

            self.stream = Some(thread::spawn(move || {
                if let Err(_) = Self::stream_handler(stream, sender) {
                    err_sender.send(AppEvent::Disconnected).unwrap();
                }
            }));
        } else {
            self.connection_string.clear();
        }
    }

    fn stream_handler(
        mut stream: TcpStream,
        sender: Sender<AppEvent>,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            let command = read_command(&mut stream)?;
            let mut event = AppEvent::Command(command.clone());

            if let Command::Transfer = command {
                let song = read_data(&mut stream)?;
                event = AppEvent::SongData(song);
            } else if let Command::Reveal = command {
                let reveal_data = read_title_grading(&mut stream)?;
                event = AppEvent::TitleGrading(reveal_data);
            }

            sender.send(event)?;
        }
    }

    fn send_nickname(&mut self, stream: &mut TcpStream) {
        let bytes = self.nickname.as_bytes();
        let num_bytes_numeric = bytes.len() as u64;
        let num_bytes = num_bytes_numeric.to_be_bytes();

        stream.write_all(&num_bytes).unwrap();
        stream.write_all(bytes).unwrap();
    }

    fn append_song(&mut self, song: Vec<u8>) -> Result<(), Box<dyn Error>> {
        self.sink.stop();
        let decoder = Decoder::new(Cursor::new(song))?;
        self.sink.append(decoder);
        self.sink.pause();
        self.state = AppState::Paused;
        Ok(())
    }

    fn play(&mut self) {
        self.state = AppState::Playing;
        self.sink.play();
    }

    fn pause(&mut self) {
        self.state = AppState::Paused;
        self.sink.pause();
    }

    fn clear(&mut self) {
        self.sink.clear();
        self.current_song = None;
    }

    fn increase_volume(&mut self) {
        self.volume += 0.05;

        if self.volume > 1.0 {
            self.volume = 1.0;
        }

        self.sink.set_volume(self.volume);
    }

    fn decrease_volume(&mut self) {
        self.volume -= 0.05;

        if self.volume < 0.0 {
            self.volume = 0.0;
        }

        self.sink.set_volume(self.volume);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = ratatui::init();

    let (_audio_stream, handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&handle)?;
    sink.set_volume(0.5);

    let (tx, rx) = mpsc::channel::<AppEvent>();

    let t1 = tx.clone();
    let t2 = tx.clone();

    thread::spawn(move || loop {
        let event = event::read().unwrap();
        t2.send(AppEvent::CrossTerm(event)).unwrap();
    });

    App {
        connection_string: String::new(),
        nickname: String::new(),
        state: AppState::EnterNickname,
        event_loop: rx,
        stream: None,
        event_sender: t1,
        current_song: None,
        current_title_grading: None,
        sink,
        volume: 0.5,
        exit: false,
    }
    .run(&mut terminal)?;

    ratatui::restore();
    Ok(())
}

fn read_command(stream: &mut TcpStream) -> Result<Command, Box<dyn Error>> {
    let mut bytes = [0_u8; 1];
    stream.read_exact(&mut bytes)?;

    let numeric = u8::from_be_bytes(bytes);

    match numeric {
        1 => Ok(Command::Play),
        2 => Ok(Command::Transfer),
        3 => Ok(Command::Pause),
        4 => Ok(Command::Repeat),
        5 => Ok(Command::Reveal),
        _ => Err(Box::new(io::Error::new(
            ErrorKind::Other,
            "Invalid Command",
        ))),
    }
}

fn read_data(stream: &mut TcpStream) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes_to_read = [0_u8; 64 / 8];
    stream.read_exact(&mut bytes_to_read)?;
    let bytes = u64::from_be_bytes(bytes_to_read);

    //println!("Server told me to revieve {} bytes", bytes);

    let mut data = vec![0_u8; bytes as usize];
    stream.read_exact(&mut data)?;

    //println!("I have read the data!");
    Ok(data)
}

fn read_title_grading(stream: &mut TcpStream) -> Result<TitleGrading, Box<dyn Error>> {
    let mut bytes_to_read = [0_u8; 64 / 8];
    stream.read_exact(&mut bytes_to_read)?;
    let bytes = u64::from_be_bytes(bytes_to_read);

    let mut data = vec![0_u8; bytes as usize];
    stream.read_exact(&mut data)?;

    let parse_string = String::from_utf8(data)?;
    let title_grading: TitleGrading = serde_json::from_str(&parse_string)?;

    Ok(title_grading)
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

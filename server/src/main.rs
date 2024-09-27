use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use serde::Deserialize;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{
        block::{Position, Title},
        Block, Paragraph, Widget,
    },
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

#[derive(Deserialize, Debug)]
struct TitleInfo {
    title: String,
    interpret: String,
}

#[derive(Deserialize, Debug)]
struct TitleList {
    titles: Vec<TitleInfo>,
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
        frame.render_widget(self, frame.area());
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
            //stream_file(&mut self.client, "/home/dominik/Documents/music/song.mp3").unwrap();
        }
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
                    format!("/home/dominik/Documents/music/{}.mp3", self.title + 1).as_str(),
                ).is_ok();
            }

            keep
        });

        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        //let title = Title::from(" Music Quiz ".bold());

        let title = Title::from(Line::from(vec![
            "Music Quiz ".into(),
            self.titles.titles[self.title as usize]
                .title
                .as_str()
                .blue(),
            " ".into(),
            self.titles.titles[self.title as usize]
                .interpret
                .as_str()
                .yellow(),
        ]));

        let instructions = Title::from(Line::from(vec![
            " Play ".into(),
            "<P>".blue().bold(),
            " Pause ".into(),
            "<O>".blue().bold(),
            " Quit ".into(),
            "<Q>".blue().bold(),
        ]));
        let block = Block::bordered()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Center)
                    .position(Position::Bottom),
            )
            .border_set(border::THICK);
        let counter_text = Text::from(vec![Line::from(vec![
            "Connected clients: ".into(),
            self.handles.lock().unwrap().len().to_string().yellow(),
            " Transfered: ".into(),
            self.transfered.to_string().yellow(),
            " Playing: ".into(),
            self.playing.to_string().yellow(),
        ])]);

        Paragraph::new(counter_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let file_content = fs::read_to_string("/home/dominik/Documents/music/titles.json")?;
    let titles: TitleList = serde_json::from_str(&file_content)?;

    let mut terminal = ratatui::init();
    let listener = TcpListener::bind("127.0.0.1:6969")?;

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

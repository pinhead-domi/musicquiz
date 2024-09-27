use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};

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
    Repeat
}

#[derive(Debug)]
struct App {
    title: u32,
    exit: bool,
    playing: bool,
    transfered: bool,
    client: TcpStream,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?
        }
        Ok(())
    }
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.match_key_event(key_event);
            }
            _ => {}
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
                Ok(_) => {},
                Err(_) => {self.exit = true;}
            }
            //stream_file(&mut self.client, "/home/dominik/Documents/music/song.mp3").unwrap();
        }
    }
    fn repeat(&mut self) {
        if self.transfered {
            self.playing = false;
            match self.send_command(Command::Repeat) {
                Ok(_) => {},
                Err(_) => {self.exit = true;}
            }
        }
    }
    fn pause(&mut self) {
        if self.playing && self.transfered {
            self.playing = false;
            match self.send_command(Command::Pause) {
                Ok(_) => {},
                Err(_) => {self.exit = true;}
            }
        }
    }
    fn transfer_file(&mut self) {
        self.send_command(Command::Transfer).unwrap();
        stream_file(&mut self.client, "/home/dominik/Documents/music/song.mp3").unwrap();
    }
    fn send_command(&mut self, command: Command) -> io::Result<()> {
        let numeric: u8 = match command {
            Command::Play => 1,
            Command::Transfer => 2,
            Command::Pause => 3,
            Command::Repeat => 4
        };

        let bytes = numeric.to_be_bytes();
        self.client.write_all(&bytes)?;

        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Title::from(" Music Quiz ".bold());
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
            "Transfered: ".into(),
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
    let mut terminal = ratatui::init();
    let listener = TcpListener::bind("127.0.0.1:6969")?;

    let client = listener.incoming().next().ok_or("Failed to open stream")?;
    let _app_result = App {
        title: 0,
        playing: false,
        client: client?,
        transfered: false,
        exit: false,
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

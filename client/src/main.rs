use std::{
    error::Error,
    io::{self, Cursor, ErrorKind, Read},
    net::TcpStream,
};

use rodio::{Decoder, OutputStream, Sink};

enum Command {
    Play,
    Transfer,
    Pause,
    Repeat
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect("localhost:6969")?;
    let (_audio_stream, handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&handle)?;
    let mut current_song: Option<Vec<u8>> = None;

    loop {
        match read_command(&mut stream)? {
            Command::Transfer => {
                println!("Got transfer command!");
                let song = read_data(&mut stream)?;
                current_song = Some(song.clone());
                append_song(&sink, song)?;
            }
            Command::Play => {
                println!("Got play command!");
                sink.play();
            },
            Command::Pause => {
                println!("Got pause command!");
                sink.pause();
            },
            Command::Repeat => {
                println!("Got repeat command!");
                if let Some(song) = current_song.clone() {
                    append_song(&sink, song)?;
                }
            }
        }
    }
}

fn append_song(sink: &Sink, song: Vec<u8>) -> Result<(),Box<dyn Error>>{
    sink.stop();
    let decoder = Decoder::new(Cursor::new(song))?;
    sink.append(decoder);
    sink.pause();
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
        _ => Err(Box::new(io::Error::new(ErrorKind::Other, "Invalid Command")))
    }
}

fn read_data(stream: &mut TcpStream) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes_to_read = [0_u8; 64 / 8];
    stream.read_exact(&mut bytes_to_read)?;
    let bytes = u64::from_be_bytes(bytes_to_read);

    println!("Server told me to revieve {} bytes", bytes);

    let mut data = vec![0_u8; bytes as usize];
    stream.read_exact(&mut data)?;

    println!("I have read the data!");
    Ok(data)
}

use std::{
    io::{self, stdout, Write},
    net::TcpStream,
    process::exit,
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo},
    event::{poll, read, Event, KeyCode, KeyModifiers},
    style::{PrintStyledContent, Stylize},
    terminal::{self, Clear, ClearType},
    QueueableCommand,
};
use game_core::constants::{LOCAL_HOST, PORT};

const _LOGO: &'static str = r#"
██████   █████  ██████   ██████   ██████   ██████  ███████ ███████ 
██   ██ ██   ██ ██   ██ ██    ██ ██    ██ ██       ██      ██      
██████  ███████ ██████  ██    ██ ██    ██ ██   ███ █████   █████   
██   ██ ██   ██ ██   ██ ██    ██ ██    ██ ██    ██ ██      ██      
██████  ██   ██ ██████   ██████   ██████   ██████  ███████ ███████ 
"#;

#[derive(Default)]
struct Client {
    stream: Option<TcpStream>,
    quit: bool,
}

impl Client {
    fn connect(&mut self, ip: &str, port: u16) {
        if self.stream.is_none() {
            self.stream = TcpStream::connect(format!("{ip}:{port}"))
                .and_then(|stream| {
                    stream.set_nonblocking(true)?;
                    Ok(stream)
                })
                .map_err(|err| eprintln!("Could not connect to {ip}:{port}, {err}"))
                .ok();
        } else {
            eprintln!("Already connected to server")
        }
    }
}

fn draw_map(stdout: &mut io::Stdout, (terminal_width, terminal_height): (u16, u16)) {
    let center_coords = (terminal_width / 2, terminal_height / 2);

    for x in 0..terminal_height - 2 {
        stdout.queue(MoveTo(0, x)).unwrap();
        stdout
            .queue(PrintStyledContent(
                "X".repeat(terminal_width as usize).grey(),
            ))
            .unwrap();
    }

    stdout
        .queue(MoveTo(center_coords.0, center_coords.1))
        .unwrap();
    stdout.queue(PrintStyledContent('P'.red())).unwrap();
}

fn draw_line(stdout: &mut io::Stdout, x: u16, w: usize) {
    stdout.queue(MoveTo(0, x)).unwrap();
    stdout
        .queue(PrintStyledContent("=".repeat(w).green()))
        .unwrap();
}

fn main() {
    terminal::enable_raw_mode().expect("failed to enable raw mode");
    let mut stdout = stdout();
    let mut terminal_dimensions = terminal::size().unwrap();

    stdout.queue(Clear(ClearType::All)).unwrap();
    stdout.queue(Hide).unwrap();

    let mut _buf = [0; 256];
    let mut client = Client::default();
    client.connect(LOCAL_HOST, PORT);

    while !client.quit {
        loop {
            while poll(Duration::ZERO).unwrap() {
                match read().unwrap() {
                    Event::Resize(w, h) => terminal_dimensions = (w, h),
                    Event::Key(event) => {
                        if let KeyCode::Char(c) = event.code {
                            if c == 'c' && event.modifiers.contains(KeyModifiers::CONTROL) {
                                terminal::disable_raw_mode().unwrap();
                                exit(0);
                            }
                        }
                    }
                    _ => {}
                }
            }

            draw_map(&mut stdout, terminal_dimensions);
            draw_line(
                &mut stdout,
                terminal_dimensions.1 - 2,
                terminal_dimensions.0 as usize,
            );

            stdout.flush().unwrap();

            thread::sleep(Duration::from_millis(33));
        }
    }
}

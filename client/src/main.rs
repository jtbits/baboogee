use std::{
    io::{self, stdout, Write, Read, ErrorKind},
    net::TcpStream,
    process::exit,
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo},
    event::{poll, read, Event, KeyCode, KeyModifiers},
    style::{PrintStyledContent, Stylize, StyledContent},
    terminal::{self, Clear, ClearType},
    QueueableCommand,
};
use game_core::{constants::{LOCAL_HOST, PORT}, protocol::{Packet, ServerPacket}, types::{Coords, Block, Map}};
use logger::{log, log_info, log_error};
use proto_dryb::Deserialize;

const _LOGO: &'static str = r#"
██████   █████  ██████   ██████   ██████   ██████  ███████ ███████ 
██   ██ ██   ██ ██   ██ ██    ██ ██    ██ ██       ██      ██      
██████  ███████ ██████  ██    ██ ██    ██ ██   ███ █████   █████   
██   ██ ██   ██ ██   ██ ██    ██ ██    ██ ██    ██ ██      ██      
██████  ██   ██ ██████   ██████   ██████   ██████  ███████ ███████ 
"#;

// TODO hack bcs cannot impl types from other crates
struct BlockWrapper(Block);
impl From<BlockWrapper> for StyledContent<char> {
    fn from(value: BlockWrapper) -> Self {
        match value.0 {
            Block::Void => ' '.grey(),
            Block::Grass => 'G'.green(),
            Block::Player => 'P'.red(),
            Block::WallHorizontal => '━'.grey(),
            Block::WallVertical => '┃'.grey(),
            Block::WallTopLeft => '┏'.grey(),
            Block::WallTopRight => '┓'.grey(),
            Block::WallBottomLeft => '┗'.grey(),
            Block::WallBottomRight => '┛'.grey(),
        }
    }
}

#[derive(Default)]
struct Client {
    stream: Option<TcpStream>,
    coords: Coords,
    visible_map: Vec<(Block, Coords)>,
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

fn get_padding(a: Coords, b: (u16, u16)) -> Coords {
    ((b.0 as i16 - a.0), (b.1 as i16 - a.1))
}

fn to_absolute((x, y): Coords, (padding_x, padding_y): Coords) -> Coords {
    (x + padding_x, y + padding_y)
}

fn draw_map(
    stdout: &mut io::Stdout,
    (terminal_width, terminal_height): (u16, u16),
    Client {
        coords: player_map_coords,
        visible_map,
        ..
    }: &Client
    ) {
    let player_terminal_coords = (terminal_width / 2, terminal_height / 2);
    let padding = get_padding(*player_map_coords, player_terminal_coords);

    // fill terminal with Block::Void
    for terminal_x in 0..terminal_width {
        for terminal_y in 0..terminal_height - 2 {
            stdout.queue(MoveTo(terminal_x, terminal_y)).unwrap();
            stdout.queue(PrintStyledContent(BlockWrapper(Block::Void).into())).unwrap();
        }
    }

    // print visible_map
    for (b, (x, y)) in visible_map
        .iter()
            .map(|&vc| (vc.0, to_absolute(vc.1, padding)))
            .collect::<Vec<_>>()
            {
                stdout.queue(MoveTo(x as u16, y as u16)).unwrap();
                stdout.queue(PrintStyledContent(BlockWrapper(b).into())).unwrap();
            }

    // print player
    stdout.queue(MoveTo(player_terminal_coords.0, player_terminal_coords.1)).unwrap();
    stdout.queue(PrintStyledContent(BlockWrapper(Block::Player).into())).unwrap();
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

    let mut buf = [0; 512];
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

            if let Some(s) = &mut client.stream {
                match s.read(&mut buf) {
                    Ok(0) => {
                        client.stream = None;
                        log_info!("Server closed the connection");
                        terminal::disable_raw_mode().unwrap();
                        exit(0);
                    },
                    Ok(n) => {
                        if let Ok((packet, size)) = Packet::deserialize(&buf[..n]) {
                            match packet {
                                Packet::Server(s) => {
                                    match s {
                                        ServerPacket::NewClientCoordsVisibleMap(nc) => {
                                            client.coords = nc.coords;
                                            client.visible_map = nc.map;
                                        }
                                    }
                                }
                            }
                        } else {
                            log_error!("Failed to deserialize server message");
                        }
                    },
                    Err(err) => {
                        if err.kind() != ErrorKind::WouldBlock {
                            client.stream = None;
                            log_error!("Connection error: {}", err);
                            exit(0);
                        }
                    },
                }
            }

            draw_map(&mut stdout, terminal_dimensions, &client);
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

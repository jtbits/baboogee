use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, stdout, ErrorKind, Read, Write},
    net::TcpStream,
    process::exit,
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo},
    event::{poll, read, Event, KeyCode, KeyModifiers},
    style::{PrintStyledContent, StyledContent, Stylize},
    terminal::{self, Clear, ClearType},
    QueueableCommand,
};
use game_core::{
    constants::{LOCAL_HOST, PORT},
    protocol::{ClientPacket, Packet, ServerPacket, Step},
    types::{Block, Coords},
};
use logger::{log, log_error, log_info};
use proto_dryb::{Deserialize, Serialize};

macro_rules! print_to_file {
    ($($arg:tt)*) => {{
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("client_state")
            .expect("Failed to open file");

        let formatted_string = format!($($arg)*);
        file.write_all(formatted_string.as_bytes())
            .expect("Failed to write to file");
    }};
}

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
            Block::Player => 'P'.blue(),
            Block::OtherPlayer => 'E'.red(),
            Block::WallHorizontal => '━'.grey(),
            Block::WallVertical => '┃'.grey(),
            Block::WallTopLeft => '┏'.grey(),
            Block::WallTopRight => '┓'.grey(),
            Block::WallBottomLeft => '┗'.grey(),
            Block::WallBottomRight => '┛'.grey(),
        }
    }
}

#[derive(Clone, Copy)]
enum PlayerState {
    InsideRadius,
    OutsideRadius,
}

impl From<PlayerState> for StyledContent<char> {
    fn from(value: PlayerState) -> Self {
        match value {
            PlayerState::InsideRadius => 'E'.red(),
            PlayerState::OutsideRadius => '?'.yellow(),
        }
    }
}

struct Player {
    id: u32,
    coords: Coords,
    state: PlayerState,
}

struct Client {
    stream: Option<TcpStream>,
    coords: Coords,
    visible_map: Vec<(Block, Coords)>,
    other_players: HashMap<u32, Player>,
    radius: u8,
    quit: bool,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            radius: 5,
            stream: Default::default(),
            coords: Default::default(),
            other_players: Default::default(),
            visible_map: Default::default(),
            quit: Default::default(),
        }
    }
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

    fn send_move(&mut self, x: char) -> Result<(), ()> {
        let packet_to_send = Packet::Client(ClientPacket::Move(Step::try_from(x)?));

        let mut buf = [0; 8];
        if let Ok(n) = packet_to_send.serialize(&mut buf) {
            if let Some(stream) = self.stream.as_mut() {
                stream.write(&mut buf[..n]).map_err(|_| ())?;
            }
        } else {
            return Err(());
        }

        Ok(())
    }

    fn remove_non_visible(&mut self) {
        self.visible_map.retain(|(_, (x, y))| {
            (x - self.coords.0).pow(2) + (y - self.coords.1).pow(2) <= (self.radius as i16).pow(2)
        });
    }

    fn update_other_player_coords_after_move(&mut self, players: &Vec<(u32, Coords)>) {
        self.update_other_player_coords_after_he_moves(players);
        for (&id, p) in self.other_players.iter_mut() {
            p.state = if players.iter().any(|p| p.0 == id) {
                PlayerState::InsideRadius
            } else {
                PlayerState::OutsideRadius
            };
        }
    }


    fn update_other_player_coords_after_he_moves(&mut self, players: &Vec<(u32, Coords)>) {
        for (id, coords) in players.iter() {
            self.other_players
                .entry(*id)
                .and_modify(|p| p.coords = *coords)
                .or_insert(Player { id: *id, coords: *coords, state: PlayerState::InsideRadius });
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
    (terminal_height, terminal_width): (u16, u16),
    Client {
        coords,
        visible_map,
        other_players,
        ..
    }: &Client,
) {
    let player_terminal_coords = (terminal_width / 2, terminal_height / 2);
    let padding = get_padding(*coords, player_terminal_coords);
    print_to_file!(
        "actual: {:?}, terminal: {:?}, padding: {:?}\n",
        coords,
        player_terminal_coords,
        padding
    );

    // fill terminal with Block::Void
    //for terminal_x in 0..terminal_height - 2 {
    //    for terminal_y in 0..terminal_width {
    //        stdout.queue(MoveTo(terminal_x, terminal_y)).expect(format!("MoveTo1, x: {}, y: {}", terminal_x, terminal_y).as_str());
    //        stdout.queue(PrintStyledContent(BlockWrapper(Block::Void).into())).expect("PrintStyledContent1");
    //    }
    //}

    print_to_file!("visible_map: ");
    for (_, (x, y)) in visible_map {
        print_to_file!("({}: {}), ", x, y);
    }
    print_to_file!("\n");

    // print visible_map
    for (b, (x, y)) in visible_map
        .iter()
        .map(|&vc| (vc.0, to_absolute(vc.1, padding)))
        .collect::<Vec<_>>()
    {
        stdout
            .queue(MoveTo(y as u16, x as u16))
            .expect(format!("MoveTo2, x: {}, y: {}", x, y).as_str());
        stdout
            .queue(PrintStyledContent(BlockWrapper(b).into()))
            .expect("PrintStyledContent2");
    }

    // print other_players
    for (state, (x, y)) in other_players
        .values()
        .map(|p| (p.state, to_absolute(p.coords, padding)))
    {
        stdout
            .queue(MoveTo(y as u16, x as u16))
            .expect(format!("MoveTo3, x: {}, y: {}", x, y).as_str());
        stdout
            .queue(PrintStyledContent(state.into()))
            .expect("PrintStyledContent3");
    }

    // print player
    stdout
        .queue(MoveTo(player_terminal_coords.1, player_terminal_coords.0))
        .expect(
            format!(
                "MoveTo4, x: {}, y: {}",
                player_terminal_coords.0, player_terminal_coords.1
            )
            .as_str(),
        );
    stdout
        .queue(PrintStyledContent(BlockWrapper(Block::Player).into()))
        .expect("PrintStyledContent4");
}

fn draw_line(stdout: &mut io::Stdout, x: u16, w: usize) {
    stdout
        .queue(MoveTo(0, x))
        .expect(format!("MoveTo5, x: {}, y: {}", 0, x).as_str());
    stdout
        .queue(PrintStyledContent("=".repeat(w).green()))
        .expect("PrintStyledContent5");
}

fn draw_coords(stdout: &mut io::Stdout, terminal_height: u16, (x, y): Coords) {
    stdout
        .queue(MoveTo(0, terminal_height))
        .expect(format!("MoveTo5, x: {}, y: {}", 0, x).as_str());
    stdout
        .queue(PrintStyledContent(format!("({:3}:{:3})", x, y).green()))
        .expect("PrintStyledContent5");
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
            while poll(Duration::ZERO).expect("poll error") {
                match read().expect("read error") {
                    Event::Resize(w, h) => terminal_dimensions = (w, h),
                    Event::Key(event) => {
                        if let KeyCode::Char(c) = event.code {
                            if c == 'c' && event.modifiers.contains(KeyModifiers::CONTROL) {
                                terminal::disable_raw_mode().unwrap();
                                stdout.queue(Clear(ClearType::All)).unwrap();
                                exit(0);
                            }

                            match c {
                                'w' | 'k' => {
                                    client
                                        .send_move(c)
                                        .expect(format!("send_move: {}", c).as_str());
                                }
                                'a' | 'h' => {
                                    client
                                        .send_move(c)
                                        .expect(format!("send_move: {}", c).as_str());
                                }
                                's' | 'j' => {
                                    client
                                        .send_move(c)
                                        .expect(format!("send_move: {}", c).as_str());
                                }
                                'd' | 'l' => {
                                    client
                                        .send_move(c)
                                        .expect(format!("send_move: {}", c).as_str());
                                }
                                _ => {}
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
                        terminal::disable_raw_mode().unwrap();
                        stdout.queue(Clear(ClearType::All)).unwrap();
                        log_info!("Server closed the connection");
                        exit(0);
                    }
                    Ok(n) => {
                        if let Ok((packet, _)) = Packet::deserialize(&buf[..n]) {
                            match packet {
                                Packet::Server(s) => {
                                    match s {
                                        ServerPacket::NewClientCoordsVisibleMap(nc) => {
                                            client.coords = nc.coords;
                                            client.visible_map = nc.map;

                                            stdout.queue(Clear(ClearType::All)).unwrap();
                                            draw_map(&mut stdout, terminal_dimensions, &client);
                                            draw_coords(
                                                &mut stdout,
                                                terminal_dimensions.0,
                                                client.coords,
                                            );
                                        }
                                        ServerPacket::NewCoords(mut nc) => {
                                            client.coords = nc.center;
                                            client.remove_non_visible();
                                            client.visible_map.append(&mut nc.coords);
                                            client.update_other_player_coords_after_move(&nc.players);
                                            //client.visible_map = nc.coords;

                                            stdout.queue(Clear(ClearType::All)).unwrap();
                                            draw_map(&mut stdout, terminal_dimensions, &client);
                                            draw_coords(
                                                &mut stdout,
                                                terminal_dimensions.0,
                                                client.coords,
                                            );
                                        }
                                        ServerPacket::OtherPlayerMoved(opm) => {
                                            client.update_other_player_coords_after_he_moves(&vec![(
                                                opm.id, opm.coords,
                                            )]);

                                            stdout.queue(Clear(ClearType::All)).unwrap();
                                            draw_map(&mut stdout, terminal_dimensions, &client);
                                            draw_coords(
                                                &mut stdout,
                                                terminal_dimensions.0,
                                                client.coords,
                                            );
                                        }
                                    }
                                }
                                _ => panic!("Server cannot send client packets"),
                            }
                        } else {
                            log_error!("Failed to deserialize server message");
                        }
                    }
                    Err(err) => {
                        if err.kind() != ErrorKind::WouldBlock {
                            client.stream = None;
                            log_error!("Connection error: {}", err);
                            exit(0);
                        }
                    }
                }
            }

            draw_line(
                &mut stdout,
                terminal_dimensions.1 - 2,
                terminal_dimensions.0 as usize,
            );

            stdout.flush().expect("flush");

            thread::sleep(Duration::from_millis(33));
        }
    }
}

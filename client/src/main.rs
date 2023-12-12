use std::{
    collections::HashMap,
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
    protocol::{self, ClientPacket, OtherPlayerMoved, Packet, ServerPacket, Step},
    types::{Block, Coords, MapCell},
    utils,
};
use logger::{log, log_error, log_info};
use proto_dryb::{Deserialize, Serialize};

#[warn(unused_macros)]
macro_rules! print_to_file {
    ($($arg:tt)*) => {{
        use std::fs::OpenOptions;

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

struct Player {
    coords: Coords,
}

struct Client {
    id: u32,
    stream: Option<TcpStream>,
    coords: Coords,
    visible_map: Vec<MapCell>,
    other_players: HashMap<u32, Player>,
    players_outside: HashMap<u32, Player>,
    radius: u8,
    quit: bool,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            radius: 5,
            id: Default::default(),
            stream: Default::default(),
            coords: Default::default(),
            other_players: Default::default(),
            players_outside: Default::default(),
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

    fn send_move(&mut self, buf: &mut [u8], x: char) -> Result<(), ()> {
        let packet_to_send = Packet::Client(ClientPacket::Move(Step::try_from(x)?));

        let n = packet_to_send.serialize(buf).map_err(|_| ())?;
        if let Some(stream) = self.stream.as_mut() {
            stream.write(&mut buf[..n]).map_err(|_| ())?;
        }

        Ok(())
    }

    fn remove_non_visible(&mut self) {
        self.visible_map.retain(|MapCell { coords: (x, y), .. }| {
            (x - self.coords.0).pow(2) + (y - self.coords.1).pow(2) <= (self.radius as i16).pow(2)
        });
    }

    fn update_other_player_coords_after_move(&mut self, players: &Vec<protocol::Player>) {
        for &protocol::Player { id, coords } in players.iter() {
            self.other_players
                .entry(id)
                .and_modify(|p| p.coords = coords)
                .or_insert(Player { coords });

            self.players_outside.remove(&id);
        }

        self.players_outside
            .retain(|_, p| !utils::is_inside_circle(self.coords, self.radius, p.coords));

        let player_to_remove = self
            .other_players
            .iter()
            .filter(|(_, p)| !utils::is_inside_circle(self.coords, self.radius, p.coords))
            .map(|(&id, _)| id)
            .collect::<Vec<_>>();

        for id in player_to_remove {
            if let Some(value) = self.other_players.remove(&id) {
                self.players_outside.insert(id, value);
            }
        }
    }

    fn update_other_player_coords_after_other_player_move(&mut self, id: u32, coords: Coords) {
        self.players_outside.remove(&id);

        self.other_players
            .entry(id)
            .and_modify(|p| p.coords = coords)
            .or_insert(Player { coords });
    }

    fn remove_player(&mut self, id: u32) {
        self.other_players.remove(&id);
        self.players_outside.remove(&id);
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
        players_outside,
        ..
    }: &Client,
) -> io::Result<()> {
    let player_terminal_coords = (terminal_width / 2, terminal_height / 2);
    let padding = get_padding(*coords, player_terminal_coords);

    // print visible_map
    for (block, (x, y)) in visible_map
        .iter()
        .map(|&MapCell { block, coords }| (block, to_absolute(coords, padding)))
    {
        stdout.queue(MoveTo(y as u16, x as u16))?;
        stdout.queue(PrintStyledContent(BlockWrapper(block).into()))?;
    }

    // print other_players
    for (x, y) in other_players
        .values()
        .map(|p| to_absolute(p.coords, padding))
    {
        stdout.queue(MoveTo(y as u16, x as u16))?;
        stdout.queue(PrintStyledContent('E'.red()))?;
    }

    // print remove players
    for (x, y) in players_outside
        .values()
        .map(|p| to_absolute(p.coords, padding))
    {
        stdout.queue(MoveTo(y as u16, x as u16))?;
        stdout.queue(PrintStyledContent('?'.yellow()))?;
    }

    // print player
    stdout.queue(MoveTo(player_terminal_coords.1, player_terminal_coords.0))?;
    stdout.queue(PrintStyledContent(BlockWrapper(Block::Player).into()))?;

    Ok(())
}

fn draw_line(stdout: &mut io::Stdout, x: u16, w: usize) -> io::Result<()> {
    stdout.queue(MoveTo(0, x))?;
    stdout.queue(PrintStyledContent("=".repeat(w).green()))?;

    Ok(())
}

fn draw_coords(stdout: &mut io::Stdout, terminal_height: u16, (x, y): Coords) -> io::Result<()> {
    stdout.queue(MoveTo(0, terminal_height))?;
    stdout.queue(PrintStyledContent(format!("({:3}:{:3})", x, y).green()))?;

    Ok(())
}

fn rerender(
    stdout: &mut io::Stdout,
    client: &Client,
    terminal_dimensions: (u16, u16),
) -> io::Result<()> {
    stdout.queue(Clear(ClearType::All))?;
    draw_map(stdout, terminal_dimensions, &client)?;
    draw_coords(stdout, terminal_dimensions.0, client.coords)?;
    draw_line(
        stdout,
        terminal_dimensions.1 - 2,
        terminal_dimensions.0 as usize,
    )?;

    Ok(())
}

fn main() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut terminal_dimensions = terminal::size()?;

    let mut stdout = stdout();
    stdout.queue(Clear(ClearType::All))?;
    stdout.queue(Hide)?;

    let mut buf = [0; 512];
    let mut client = Client::default();
    client.connect(LOCAL_HOST, PORT);

    while !client.quit {
        loop {
            while poll(Duration::ZERO)? {
                handle_io_read(&mut stdout, &mut client, &mut terminal_dimensions, &mut buf)?;
            }

            if let Some(mut s) = client.stream.take() {
                handle_tcp_read(
                    &mut s,
                    &mut stdout,
                    &mut client,
                    terminal_dimensions,
                    &mut buf,
                )?;
                client.stream = Some(s);
            }

            stdout.flush()?;

            thread::sleep(Duration::from_millis(33));
        }
    }

    Ok(())
}

fn handle_io_read(
    stdout: &mut io::Stdout,
    client: &mut Client,
    terminal_dimensions: &mut (u16, u16),
    buf: &mut [u8],
) -> io::Result<()> {
    match read()? {
        Event::Resize(w, h) => {
            terminal_dimensions.0 = w;
            terminal_dimensions.1 = h;
        }
        Event::Key(event) => {
            if let KeyCode::Char(c) = event.code {
                if c == 'c' && event.modifiers.contains(KeyModifiers::CONTROL) {
                    terminal::disable_raw_mode()?;
                    stdout.queue(Clear(ClearType::All))?;
                    exit(0);
                }

                match c {
                    'w' | 'k' | 'a' | 'h' | 's' | 'j' | 'd' | 'l' => client
                        .send_move(buf, c)
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "send move"))?,
                    _ => {}
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn handle_tcp_read(
    s: &mut TcpStream,
    stdout: &mut io::Stdout,
    client: &mut Client,
    terminal_dimensions: (u16, u16),
    buf: &mut [u8],
) -> io::Result<()> {
    match s.read(buf) {
        Ok(0) => {
            client.stream = None;
            terminal::disable_raw_mode()?;
            stdout.queue(Clear(ClearType::All))?;
            log_info!("Server closed the connection");
            exit(0);
        }
        Ok(n) => {
            if let Ok((packet, _)) = Packet::deserialize(&buf[..n]) {
                handle_packet(packet, stdout, client, terminal_dimensions)?;
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

    Ok(())
}

fn handle_packet(
    packet: Packet,
    stdout: &mut io::Stdout,
    client: &mut Client,
    terminal_dimensions: (u16, u16),
) -> io::Result<()> {
    match packet {
        Packet::Server(s) => match s {
            ServerPacket::NewClientCoordsVisibleMap(nc) => {
                client.id = nc.id;
                client.coords = nc.coords;
                client.visible_map = nc.map.into_iter().map(|mc| mc.into()).collect();
                client.other_players = nc
                    .players
                    .into_iter()
                    .map(|p| (p.id, Player { coords: p.coords }))
                    .collect();
            }
            ServerPacket::NewCoords(nc) => {
                client.coords = nc.center;
                client.remove_non_visible();
                client
                    .visible_map
                    .append(&mut nc.coords.into_iter().map(|mc| mc.into()).collect());
                client.update_other_player_coords_after_move(&nc.players);
            }
            ServerPacket::OtherPlayerMoved(OtherPlayerMoved { id, coords }) => {
                client.update_other_player_coords_after_other_player_move(id, coords);
            }
            ServerPacket::OtherPlayerMovedOutsideRadius(id)
            | ServerPacket::PlayerDisconnected(id) => {
                client.remove_player(id);
            }
        },
        _ => panic!("Server cannot send client packets"),
    }

    rerender(stdout, client, terminal_dimensions)?;

    Ok(())
}

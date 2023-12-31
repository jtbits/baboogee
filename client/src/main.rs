use std::{
    cmp::{max, min},
    collections::HashMap,
    io::{self, stdout, ErrorKind, Read, Write},
    net::{TcpStream, Shutdown},
    process::exit,
    sync::{
        mpsc::{channel, Sender},
        Arc, Mutex, RwLock,
    },
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
    protocol::{self, ClientPacket, Direction, OtherPlayerMoved, Packet, ServerPacket},
    types::{Block, Coords, MapCell},
    utils,
};
use logger::{log, log_error, log_info};
use proto_dryb::{Deserialize, Serialize};

#[allow(unused_macros)]

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

// https://www.patorjk.com/software/taag/#p=display&f=ANSI%20Regular&t=BABOOGEE
const LOGO: &'static str = r#"
██████   █████  ██████   ██████   ██████   ██████  ███████ ███████ 
██   ██ ██   ██ ██   ██ ██    ██ ██    ██ ██       ██      ██      
██████  ███████ ██████  ██    ██ ██    ██ ██   ███ █████   █████   
██   ██ ██   ██ ██   ██ ██    ██ ██    ██ ██    ██ ██      ██      
██████  ██   ██ ██████   ██████   ██████   ██████  ███████ ███████ 
"#;

const KILLED_YOU: &'static str = r#"
██   ██ ██ ██      ██      ███████ ██████      ██    ██  ██████  ██    ██ 
██  ██  ██ ██      ██      ██      ██   ██      ██  ██  ██    ██ ██    ██ 
█████   ██ ██      ██      █████   ██   ██       ████   ██    ██ ██    ██ 
██  ██  ██ ██      ██      ██      ██   ██        ██    ██    ██ ██    ██ 
██   ██ ██ ███████ ███████ ███████ ██████         ██     ██████   ██████  
"#;

const ONE: &'static str = r#"
 ██
███
 ██
 ██
 ██
"#;

const TWO: &'static str = r#"
██████  
     ██ 
 █████  
██      
███████
"#;

const THREE: &'static str = r#"
██████ 
     ██
 █████ 
     ██
██████ 
"#;

const FOUR: &'static str = r#"
██   ██
██   ██
███████
     ██
     ██
"#;

const FIVE: &'static str = r#"
███████
██     
███████
     ██
███████
"#;

const SIX: &'static str = r#"
 ██████ 
██      
███████ 
██    ██
 ██████ 
"#;

const SEVEN: &'static str = r#"
███████
     ██
    ██ 
   ██  
   ██
"#;

const EIGHT: &'static str = r#"
 █████ 
██   ██
 █████ 
██   ██
 █████ 
"#;

const NINE: &'static str = r#"
 █████ 
██   ██
 ██████
     ██
 █████ 
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
    max_hp: u8,
    current_hp: u8,
    weapon: Weapon,
    shooting_angle: Direction,
    quit: bool,
}

#[derive(Default)]
struct Weapon {
    range: u8,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            radius: 5,
            shooting_angle: Direction::Up,

            id: 0,
            max_hp: 0,
            current_hp: 0,
            quit: false,
            stream: None,
            visible_map: vec![],
            other_players: HashMap::default(),
            players_outside: HashMap::default(),
            weapon: Weapon::default(),
            coords: Default::default(),
        }
    }
}

impl Client {
    fn send_move(&mut self, buf: &mut [u8], x: char) -> Result<(), ()> {
        let packet_to_send = Packet::Client(ClientPacket::Move(Direction::try_from(x)?));

        let n = packet_to_send.serialize(buf).map_err(|_| ())?;
        if let Some(stream) = self.stream.as_mut() {
            stream.write(&mut buf[..n]).map_err(|_| ())?;
        }

        Ok(())
    }

    fn send_shoot(&mut self, buf: &mut [u8]) -> Result<(), ()> {
        let packet_to_send = Packet::Client(ClientPacket::Shoot(self.shooting_angle));

        let n = packet_to_send.serialize(buf).map_err(|_| ())?;
        if let Some(stream) = self.stream.as_mut() {
            stream.write(&mut buf[..n]).map_err(|_| ())?;
        }

        Ok(())
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

    fn remove_non_visible(&mut self) {
        self.visible_map.retain(|&MapCell { coords: (x, y), .. }| {
            (x as i16 - self.coords.0 as i16).pow(2) + (y as i16 - self.coords.1 as i16).pow(2)
                <= (self.radius as i16).pow(2)
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

fn get_padding(a: Coords, b: Coords) -> (i16, i16) {
    ((b.0 as i16 - a.0 as i16), (b.1 as i16 - a.1 as i16))
}

fn to_absolute((x, y): Coords, (padding_x, padding_y): (i16, i16)) -> Coords {
    ((x as i16 + padding_x) as u16, (y as i16 + padding_y) as u16)
}

fn draw_map(
    stdout: &Arc<Mutex<io::Stdout>>,
    (terminal_height, terminal_width): (u16, u16),
    client: &Arc<RwLock<Client>>,
) -> io::Result<()> {
    let player_terminal_coords = (terminal_width / 2, terminal_height / 2);
    let client = client.read().unwrap();
    let padding = get_padding(client.coords, player_terminal_coords);

    let mut stdout = stdout.lock().unwrap();

    // print visible_map
    for (block, (x, y)) in client
        .visible_map
        .iter()
        .map(|&MapCell { block, coords }| (block, to_absolute(coords, padding)))
    {
        stdout.queue(MoveTo(y as u16, x as u16))?;
        stdout.queue(PrintStyledContent(BlockWrapper(block).into()))?;
    }

    // print other_players
    for (x, y) in client
        .other_players
        .values()
        .map(|p| to_absolute(p.coords, padding))
    {
        stdout.queue(MoveTo(y as u16, x as u16))?;
        stdout.queue(PrintStyledContent('E'.red()))?;
    }

    // print remove players
    for (x, y) in client
        .players_outside
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

fn draw_metadata(stdout: &Arc<Mutex<io::Stdout>>, client: &Arc<RwLock<Client>>) -> io::Result<()> {
    let mut stdout = stdout.lock().unwrap();
    let client = client.read().unwrap();
    let (x, y) = client.coords;
    stdout.queue(MoveTo(0, 0))?;
    stdout.queue(PrintStyledContent(format!("XY ({:2}:{:2})", x, y).green()))?;
    stdout.queue(MoveTo(0, 1))?;
    stdout.queue(PrintStyledContent(
        format!("HP ({:2}/{:2})", client.current_hp, client.max_hp).red(),
    ))?;

    Ok(())
}

fn rerender(
    stdout: &Arc<Mutex<io::Stdout>>,
    client: &Arc<RwLock<Client>>,
    terminal_dimensions: (u16, u16),
) -> io::Result<()> {
    {
        let mut stdout = stdout.lock().unwrap();
        stdout.queue(Clear(ClearType::All))?;
    }
    draw_map(stdout, terminal_dimensions, client)?;
    draw_metadata(stdout, client)?;

    Ok(())
}

enum CommandEnum {
    MoveTo(i8, i8),
    PrintStyledContent(StyledContent<char>),
    ReRender,
}

fn main() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut terminal_dimensions = terminal::size()?;

    let stdout = Arc::new(Mutex::new(stdout()));
    configure_stdout(&stdout)?;

    print_logo_scene(&stdout, terminal_dimensions)?;

    let mut buf = [0; 512];
    let client = Arc::new(RwLock::new(Client::default()));
    {
        client.write().unwrap().connect(LOCAL_HOST, PORT);
    }

    let (animation_sender, animation_receiver) = channel::<Vec<CommandEnum>>();
    let stdout_animation_recv = stdout.clone();
    let client_animation_recv = client.clone();
    thread::spawn(move || {
        let stdout = Arc::clone(&stdout_animation_recv);
        let client = Arc::clone(&client_animation_recv);

        loop {
            match animation_receiver.recv() {
                Ok(events) => {
                    for event in events {
                        match event {
                            CommandEnum::MoveTo(y, x) => {
                                let (w, h) = terminal::size().unwrap();
                                // TODO god bless me refactor this pls my brain hurts a lot
                                let (y, x) = (
                                    ((w / 2) as i16 + y as i16) as u16,
                                    ((h / 2) as i16 + x as i16) as u16,
                                );
                                let mut stdout = stdout.lock().unwrap();
                                stdout.queue(MoveTo(y, x)).unwrap();
                            }
                            CommandEnum::PrintStyledContent(symbol) => {
                                let mut stdout = stdout.lock().unwrap();
                                stdout.queue(PrintStyledContent(symbol)).unwrap();
                            }
                            CommandEnum::ReRender => {
                                rerender(&stdout, &client, terminal::size().unwrap()).unwrap();
                            }
                        }
                    }
                    let mut stdout = stdout.lock().unwrap();
                    stdout.flush().unwrap();
                }
                Err(_err) => {}
            }

            if let Ok(client) = client.read() {
                if client.quit {
                    break;
                }
            }
        }
    });

    loop {
        while poll(Duration::ZERO)? {
            handle_io_read(
                &stdout,
                &client,
                &mut terminal_dimensions,
                &mut buf,
                &animation_sender,
            )?;
        }

        // FIXME i hate this approach dont need to take and return the stream + wont need to drop
        // stream at the end of the loop
        let stream = {
            let mut client = client.write().unwrap();
            client.stream.take()
        };
        if let Some(mut s) = stream {
            handle_tcp_read(&mut s, &stdout, &client, terminal_dimensions, &mut buf)?;
            let mut client = client.write().unwrap();
            client.stream = Some(s);
        }

        let mut stdout = stdout.lock().unwrap();
        stdout.flush()?;

        thread::sleep(Duration::from_millis(33));

        if let Ok(mut client) = client.write() {
            if client.quit == true {
                if let Some(s) = client.stream.take() {
                    drop(s);
                }
                break;
            }
        }
    }

    draw_death_scene(&stdout, terminal_dimensions)?;

    Ok(())
}

fn print_logo_scene(
    stdout: &Arc<Mutex<io::Stdout>>,
    (terminal_width, terminal_height): Coords,
) -> io::Result<()> {
    let mut stdout = stdout.lock().unwrap();

    let logo_as_2d_vec = LOGO
        .lines()
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let height = logo_as_2d_vec.len() as i16;
    let max_width = logo_as_2d_vec.iter().map(|row| row.len()).max().unwrap() as i16;
    let mid_terminal_height = terminal_height / 2;

    for w in (-max_width..terminal_width as i16).rev() {
        for (h, i) in (-height / 2..height / 2).zip(0..height) {
            let h = (mid_terminal_height as i16 + h) as u16;
            stdout.queue(MoveTo(max(w, 0) as u16, h))?;
            let row = &logo_as_2d_vec[i as usize];
            let row_width = row.len();
            let row_slice = &row[max(0, min(row_width as i16, -w)) as usize
                ..min((terminal_width - max(w, 0) as u16) as usize, row.len())];
            stdout.queue(PrintStyledContent(
                row_slice.into_iter().collect::<String>().red(),
            ))?;
        }
        stdout.flush()?;
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

fn draw_death_scene(
    stdout: &Arc<Mutex<io::Stdout>>,
    (terminal_width, terminal_height): Coords,
) -> io::Result<()> {
    let mut stdout = stdout.lock().unwrap();

    stdout.queue(Clear(ClearType::All))?;

    let logo_as_2d_vec = KILLED_YOU
        .lines()
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let height = logo_as_2d_vec.len() as i16;
    let max_width = logo_as_2d_vec.iter().map(|row| row.len()).max().unwrap() as i16;
    let mid_terminal_height = terminal_height / 2;

    for w in (-max_width..terminal_width as i16).rev() {
        for (h, i) in (-height / 2..height / 2).zip(0..height) {
            let h = (mid_terminal_height as i16 + h) as u16;
            stdout.queue(MoveTo(max(w, 0) as u16, h))?;
            let row = &logo_as_2d_vec[i as usize];
            let row_width = row.len();
            let row_slice = &row[max(0, min(row_width as i16, -w)) as usize
                ..min((terminal_width - max(w, 0) as u16) as usize, row.len())];
            stdout.queue(PrintStyledContent(
                row_slice.into_iter().collect::<String>().red(),
            ))?;
        }
        stdout.flush()?;
        thread::sleep(Duration::from_millis(33));
    }

    thread::sleep(Duration::from_millis(333));

    Ok(())
}

fn configure_stdout(stdout: &Arc<Mutex<io::Stdout>>) -> io::Result<()> {
    let mut stdout = stdout.lock().unwrap();

    stdout.queue(Clear(ClearType::All))?;
    stdout.queue(Hide)?;

    Ok(())
}

fn handle_io_read(
    stdout: &Arc<Mutex<io::Stdout>>,
    client: &Arc<RwLock<Client>>,
    terminal_dimensions: &mut (u16, u16),
    buf: &mut [u8],
    animation_sender: &Sender<Vec<CommandEnum>>,
) -> io::Result<()> {
    match read()? {
        Event::Resize(w, h) => {
            terminal_dimensions.0 = w;
            terminal_dimensions.1 = h;
        }
        Event::Key(event) => {
            if let KeyCode::Char(c) = event.code {
                if c == 'c' && event.modifiers.contains(KeyModifiers::CONTROL) {
                    let mut stdout = stdout.lock().unwrap();
                    terminal::disable_raw_mode()?;
                    stdout.queue(Clear(ClearType::All))?;
                    exit(0);
                }

                match c {
                    'w' | 'k' | 'a' | 'h' | 's' | 'j' | 'd' | 'l' => {
                        let mut client = client.write().unwrap();
                        client
                            .send_move(buf, c)
                            .map_err(|_| io::Error::new(io::ErrorKind::Other, "send move"))?;

                        client.shooting_angle = match c {
                            'w' | 'k' => Direction::Up,
                            's' | 'j' => Direction::Down,
                            'a' | 'h' => Direction::Left,
                            'd' | 'l' => Direction::Right,
                            _ => unreachable!(),
                        }
                    }
                    ' ' => {
                        let mut client = client.write().unwrap();
                        client
                            .send_shoot(buf)
                            .map_err(|_| io::Error::new(io::ErrorKind::Other, "send shoot"))?;

                        let shooting_angle = client.shooting_angle;
                        let range = client.weapon.range as i8;
                        let (absolute_x, absolute_y) = (0, 0);
                        let animation_sender = animation_sender.clone();
                        thread::spawn(move || {
                            let point_from = match shooting_angle {
                                Direction::Up => (absolute_x - 1, absolute_y),
                                Direction::Down => (absolute_x + 1, absolute_y),
                                Direction::Left => (absolute_x, absolute_y - 1),
                                Direction::Right => (absolute_x, absolute_y + 1),
                            };

                            let point_to = match shooting_angle {
                                Direction::Up => (point_from.0 - range, point_from.1),
                                Direction::Down => (point_from.0 + range, point_from.1),
                                Direction::Left => (point_from.0, point_from.1 - range),
                                Direction::Right => (point_from.0, point_from.1 + range),
                            };

                            let symbol = match shooting_angle {
                                Direction::Up | Direction::Down => '║',
                                Direction::Left | Direction::Right => '═',
                            };

                            match shooting_angle {
                                Direction::Right | Direction::Down => {
                                    for x in point_from.0..=point_to.0 {
                                        for y in point_from.1..=point_to.1 {
                                            animation_sender
                                                .send(vec![
                                                    CommandEnum::MoveTo(y, x),
                                                    CommandEnum::PrintStyledContent(
                                                        symbol.dark_red(),
                                                    ),
                                                ])
                                                .unwrap();

                                            thread::sleep(Duration::from_millis(33));
                                        }
                                    }
                                    animation_sender.send(vec![CommandEnum::ReRender]).unwrap();
                                }
                                Direction::Up | Direction::Left => {
                                    for x in (point_to.0..=point_from.0).rev() {
                                        for y in (point_to.1..=point_from.1).rev() {
                                            animation_sender
                                                .send(vec![
                                                    CommandEnum::MoveTo(y, x),
                                                    CommandEnum::PrintStyledContent(
                                                        symbol.dark_red(),
                                                    ),
                                                ])
                                                .unwrap();

                                            thread::sleep(Duration::from_millis(33));
                                        }
                                    }
                                    animation_sender.send(vec![CommandEnum::ReRender]).unwrap();
                                }
                            }
                        });
                    }
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
    stdout: &Arc<Mutex<io::Stdout>>,
    client: &Arc<RwLock<Client>>,
    terminal_dimensions: (u16, u16),
    buf: &mut [u8],
) -> io::Result<()> {
    match s.read(buf) {
        Ok(0) => {
            let mut client = client.write().unwrap();
            client.stream = None;
            terminal::disable_raw_mode()?;
            let mut stdout = stdout.lock().unwrap();
            stdout.queue(Clear(ClearType::All))?;
            log_info!("Server closed the connection");
            exit(0);
        }
        Ok(n) => {
            if let Ok((packet, _)) = Packet::deserialize(&buf[..n]) {
                handle_packet(packet, stdout, &client, terminal_dimensions)?;
            } else {
                log_error!("Failed to deserialize server message");
            }
        }
        Err(err) => {
            if err.kind() != ErrorKind::WouldBlock {
                let mut client = client.write().unwrap();
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
    stdout: &Arc<Mutex<io::Stdout>>,
    client: &Arc<RwLock<Client>>,
    terminal_dimensions: (u16, u16),
) -> io::Result<()> {
    {
        let mut client = client.write().unwrap();
        match packet {
            Packet::Server(s) => match s {
                ServerPacket::NewClientCoordsVisibleMap(nc) => {
                    client.id = nc.id;
                    client.coords = nc.coords;
                    client.max_hp = nc.hp;
                    client.current_hp = nc.hp;
                    client.radius = nc.radius;
                    client.weapon.range = nc.weapon_range;
                    client.visible_map =
                        nc.visible_coords.into_iter().map(|mc| mc.into()).collect();
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
                ServerPacket::PlayerWasShot(damage, _direction) => {
                    client.current_hp = client.current_hp.checked_sub(damage).unwrap_or(0);
                }
                ServerPacket::PlayerDied(_id) => {
                    client.quit = true;
                }
            },
            _ => panic!("Server cannot send client packets"),
        }
    }

    rerender(stdout, client, terminal_dimensions)?;

    Ok(())
}

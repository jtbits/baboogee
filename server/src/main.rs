use std::{
    cmp::min,
    collections::HashMap,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    ops::Deref,
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

use game_core::{
    constants,
    protocol::{self, ClientPacket, Direction, Packet, Player},
    types::{self, Block, Coords, Map},
    utils,
};
use logger::{log, log_error, log_info};
use proto_dryb::Deserialize;

const PREDICATE_CLIENT_INSIDE_RADIUS: fn(Coords, u8, Coords) -> bool =
    |c1_coords, c1_radius, c2_coords| utils::is_inside_circle(c1_coords, c1_radius, c2_coords);
const BUF_SIZE_512: usize = 512;
const _BUF_SIZE_256: usize = 256;
const _BUF_SIZE_128: usize = 128;
const _BUF_SIZE_64: usize = 64;
const _BUF_SIZE_32: usize = 32;
const BUF_SIZE_16: usize = 16;
const BUF_SIZE_8: usize = 8;

enum ClientEvent {
    Connect {
        addr: SocketAddr,
        stream: Arc<TcpStream>,
    },
    Disconnect {
        addr: SocketAddr,
    },
    Read {
        addr: SocketAddr,
        bytes: Box<[u8]>,
    },
    Error {
        addr: SocketAddr,
        err: io::Error,
    },
}

struct Client {
    conn: Arc<TcpStream>,

    id: u32,
    coords: Coords,
    radius: u8,
    hp: u8,
    weapon: Weapon,

    map_ref: Arc<RwLock<ServerMap>>,
}

struct Weapon {
    range: u8,
    _pierce: u8,
    damage: u8,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            range: 5,
            damage: 1,
            _pierce: 1,
        }
    }
}

impl Client {
    fn new_from_conn(conn: Arc<TcpStream>, id: &mut u32, map: &Arc<RwLock<ServerMap>>) -> Self {
        let (max_x, max_y) = {
            let map = map.read().unwrap();
            (map.height, map.width)
        };
        let coords = utils::generate_random_coords(max_x, max_y);
        let new = Self {
            conn,
            coords,
            weapon: Weapon::default(),
            radius: 5,
            hp: 10,
            id: *id,
            map_ref: Arc::clone(map),
        };
        *id += 1;
        new
    }

    fn do_shoot(&self, direction: Direction, buf: &mut [u8]) {
        let &Client {
            coords: (x, y),
            weapon: Weapon { range, damage, .. },
            ..
        } = self;
        let (h, w) = {
            let map = self.map_ref.read().unwrap();
            (map.height as u16, map.width as u16)
        };
        let point_from = match direction {
            Direction::Up => (x.checked_sub(1).unwrap_or(0), y),
            Direction::Down => (min(x + 1, h), y),
            Direction::Left => (x, y.checked_sub(1).unwrap_or(0)),
            Direction::Right => (x, min(y + 1, w)),
        };
        let range = range as u16;
        let point_to = match direction {
            Direction::Up => (x.checked_sub(1 + range).unwrap_or(0), y),
            Direction::Down => (min(x + 1 + range, h), y),
            Direction::Left => (x, y.checked_sub(1 + range).unwrap_or(0)),
            Direction::Right => (x, min(y + 1 + range, w)),
        };

        match direction {
            Direction::Down | Direction::Right => {
                let map = self.map_ref.read().unwrap();
                for i in point_from.0..=point_to.0 {
                    for j in point_from.1..=point_to.1 {
                        if let Some(ref enemy) = map.coords[i as usize][j as usize].client {
                            let mut enemy = enemy.write().unwrap();
                            enemy.hp = enemy.hp.checked_sub(damage).unwrap_or(0);

                            if enemy.hp == 0 {
                                log_info!("Player: {} died", enemy.id);
                                let n =
                                    protocol::generate_player_died_payload(buf, self.id).unwrap();
                                let _ = enemy.write(&buf[..n]);
                                return;
                            }

                            let n =
                                protocol::generate_shoot_payload(buf, damage, direction).unwrap();
                            let _ = enemy.write(&buf[..n]);
                            return;
                        }
                    }
                }
            }
            Direction::Up | Direction::Left => {
                let map = self.map_ref.read().unwrap();
                for i in (point_to.0..=point_from.0).rev() {
                    for j in (point_to.1..=point_from.1).rev() {
                        if let Some(ref enemy) = map.coords[i as usize][j as usize].client {
                            let mut enemy = enemy.write().unwrap();
                            enemy.hp = enemy.hp.checked_sub(damage).unwrap_or(0);

                            if enemy.hp == 0 {
                                log_info!("Player: {} died", enemy.id);
                                let n =
                                    protocol::generate_player_died_payload(buf, self.id).unwrap();
                                let _ = enemy.write(&buf[..n]);
                                return;
                            }

                            let n =
                                protocol::generate_shoot_payload(buf, damage, direction).unwrap();
                            let _ = enemy.write(&buf[..n]);
                            return;
                        }
                    }
                }
            }
        }
    }

    fn do_move(
        &mut self,
        direction: Direction,
        clients: &HashMap<SocketAddr, Arc<RwLock<Client>>>,
        buf: &mut [u8],
    ) -> Result<(), String> {
        let prev_coords = self.coords;
        let (new_x, new_y) = match direction {
            Direction::Up => (
                self.coords.0.checked_sub(1).ok_or("Cannot move up")?,
                self.coords.1,
            ),
            Direction::Down => (self.coords.0 + 1, self.coords.1),
            Direction::Left => (
                self.coords.0,
                self.coords.1.checked_sub(1).ok_or("Cannot move left")?,
            ),
            Direction::Right => (self.coords.0, self.coords.1 + 1),
        };

        {
            let map = self.map_ref.read().unwrap();

            // Check map bounds
            if new_x >= map.height as u16 || new_y >= map.width as u16 {
                return Err("New position is outside the map".to_string());
            }

            // Check if new cell is occupied
            if map
                .coords
                .get(new_x as usize)
                .and_then(|row| row.get(new_y as usize))
                .and_then(|cell| cell.client.as_ref())
                .is_some()
            {
                return Err("Cell is occupied".to_string());
            }
        }

        // Perform the move
        {
            let mut map = self.map_ref.write().unwrap();
            let current_cell = std::mem::replace(
                &mut map.coords[self.coords.0 as usize][self.coords.1 as usize].client,
                None,
            );
            map.coords[new_x as usize][new_y as usize].client = current_cell;
        }

        self.coords = (new_x, new_y);

        let mut buf_move = [0; BUF_SIZE_16];
        let n_move = protocol::generate_move_notify_payload(&mut buf_move, self.coords, self.id)
            .map_err(|_| "Error during generating payload move notify")?;
        let mut buf_move_outside = [0; BUF_SIZE_8];
        let n_move_outside =
            protocol::generate_move_outside_radius_notify_payload(&mut buf_move_outside, self.id)
                .map_err(|_| "Error during generating payload move outside radius")?;
        let mut visible_players_to_client = vec![];
        for c in clients.values() {
            {
                if let Err(_) = c.try_read() {
                    continue;
                }
                //if c.id == self.id {
                //    continue;
                //}
            }
            let mut c = c.write().unwrap();

            if PREDICATE_CLIENT_INSIDE_RADIUS(self.coords, self.radius, c.coords) {
                visible_players_to_client.push(Player::new(c.id, c.coords))
            }

            // send to other players new coords of this if in radius
            if PREDICATE_CLIENT_INSIDE_RADIUS(c.coords, c.radius, self.coords) {
                let _ = c.write(&buf_move[..n_move]);
            }

            // sent to other players if player moved outside from their radius
            if PREDICATE_CLIENT_INSIDE_RADIUS(c.coords, c.radius, prev_coords)
                && !PREDICATE_CLIENT_INSIDE_RADIUS(c.coords, c.radius, self.coords)
            {
                let _ = c.write(&buf_move_outside[..n_move_outside]);
            }
        }
        // send new coords to player
        let new_visiple_coord = visible_map(&self.map_ref, self.coords, self.radius);
        let n = protocol::generate_new_coords_payload(
            buf,
            self.coords,
            new_visiple_coord,
            visible_players_to_client,
        )
        .map_err(|_| "Error during generating payload for new coords")?;
        let _ = self.write(&buf[..n]);

        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.conn.deref().write(buf)
    }
}

struct MapCell {
    block: Block,
    client: Option<Arc<RwLock<Client>>>,
}

impl Default for MapCell {
    fn default() -> Self {
        Self {
            block: Block::Grass,
            client: None,
        }
    }
}

struct ServerMap {
    height: usize,
    width: usize,
    coords: Vec<Vec<MapCell>>,
}

impl ServerMap {
    fn from_map(map: &Map) -> ServerMap {
        let &Map { height, width, .. } = map;
        let coords = &map.coords;
        let mut sm_coords = Vec::with_capacity(height);
        for _ in 0..height {
            sm_coords.push(Vec::with_capacity(width));
        }

        for (i, row) in sm_coords.iter_mut().enumerate() {
            for j in 0..row.capacity() {
                row.push(MapCell {
                    block: coords[i][j],
                    client: None,
                });
            }
        }

        ServerMap {
            height,
            width,
            coords: sm_coords,
        }
    }
}

struct Server {
    clients: HashMap<SocketAddr, Arc<RwLock<Client>>>,
    id_counter: u32,
    map: Arc<RwLock<ServerMap>>,
}

impl Server {
    fn new() -> Self {
        let map = ServerMap::from_map(&utils::generate_map());
        Self {
            map: Arc::new(RwLock::new(map)),
            id_counter: 0,
            clients: HashMap::new(),
        }
    }

    fn client_connected(
        &mut self,
        buf: &mut [u8],
        addr: SocketAddr,
        stream: Arc<TcpStream>,
    ) -> Result<(), ()> {
        log_info!("Client {addr} connected");

        let client = Client::new_from_conn(stream, &mut self.id_counter, &self.map);

        let players_inside_radius = self
            .clients
            .values()
            .filter(|&c| {
                utils::is_inside_circle(client.coords, client.radius, c.read().unwrap().coords)
            })
            .map(|c| Player::new(c.read().unwrap().id, c.read().unwrap().coords))
            .collect::<Vec<_>>();

        let visible_coords = visible_map(&self.map, client.coords, client.radius);
        let n = protocol::generate_initial_payload(
            buf,
            client.id,
            client.coords,
            client.radius,
            client.hp,
            client.weapon.range,
            visible_coords,
            players_inside_radius,
        )
        .map_err(|_| log_error!("Could not generate payload"))?;

        client
            .conn
            .deref()
            .write(&buf[..n])
            .map_err(|err| log_error!("Could not write to client: {addr}, {err}"))?;

        let players_seeing_client = self.clients.iter().filter(|(_, c)| {
            let c = c.read().unwrap();
            utils::is_inside_circle(c.coords, c.radius, client.coords)
        });

        for (&other_addr, other_client) in players_seeing_client {
            log_info!(
                "Sending move notification to player with id: {}",
                other_client.read().unwrap().id
            );
            let n = protocol::generate_move_notify_payload(buf, client.coords, client.id)
                .map_err(|_| ())?;
            other_client
                .read()
                .unwrap()
                .conn
                .deref()
                .write(&buf[..n])
                .map_err(|err| {
                    log_error!("Could not notify client {other_addr} about the move: {err}")
                })?;
        }

        let (x, y) = (client.coords.0 as usize, client.coords.1 as usize);
        let client = Arc::new(RwLock::new(client));
        self.map
            .write()
            .unwrap()
            .coords
            .get_mut(x)
            .and_then(|row| row.get_mut(y))
            .map(|col| col.client = Some(Arc::clone(&client)));
        self.clients.insert(addr, client);

        Ok(())
    }

    fn client_disconnected(&mut self, addr: SocketAddr, buf: &mut [u8]) -> Result<(), ()> {
        log_info!("Client {addr} disconnected");

        let (id, coords) = {
            let removed = self
                .clients
                .remove(&addr)
                .ok_or(())
                .map_err(|_| log_error!("Did not found client in hashmap on disconnect"))?;
            let removed = removed.read().unwrap();

            (removed.id, removed.coords)
        };

        self.map
            .write()
            .unwrap()
            .coords
            .get_mut(coords.0 as usize)
            .and_then(|row| row.get_mut(coords.1 as usize))
            .map(|mc| mc.client = None);

        let n = protocol::generate_player_disconnected(buf, id)
            .map_err(|_| log_error!("Could not generate player_disconnected"))?;

        for c in self.clients.values() {
            let _ = c
                .write()
                .unwrap()
                .write(&mut buf[..n]);
        }

        Ok(())
    }

    fn client_wrote(&mut self, addr: SocketAddr, bytes: &[u8], buf: &mut [u8]) -> Result<(), ()> {
        let client = self.clients.get(&addr).ok_or(()).map_err(|_| ())?;
        let (packet, _) = Packet::deserialize(bytes)
            .map_err(|_| log_error!("Could not deserialize packet from client"))?;

        match packet {
            Packet::Client(cp) => match cp {
                ClientPacket::Shoot(direction) => {
                    client.read().unwrap().do_shoot(direction, buf);
                }
                ClientPacket::Move(direction) => {
                    log_info!("Got Move client packet with direction: {:?}", direction);
                    if let Err(err) = client
                        .write()
                        .unwrap()
                        .do_move(direction, &self.clients, buf)
                    {
                        log_error!("Client {addr} can not move, err: {err}");
                    }
                }
            },
            _ => return Err(()),
        }

        Ok(())
    }
}

fn server(events: Receiver<ClientEvent>) -> Result<(), ()> {
    let mut server = Server::new();
    let mut buf = [0; BUF_SIZE_512];

    loop {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(msg) => match msg {
                ClientEvent::Connect { addr, stream } => {
                    server.client_connected(&mut buf, addr, stream)?
                }
                ClientEvent::Disconnect { addr } => server.client_disconnected(addr, &mut buf)?,
                ClientEvent::Read { addr, bytes } => server.client_wrote(addr, &bytes, &mut buf)?,
                ClientEvent::Error { addr, err } => log_error!("Client error: {}, {}", addr, err),
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                log_error!("Message receiver disconnected");
                return Err(());
            }
        }
    }
}

fn client(stream: Arc<TcpStream>, addr: SocketAddr, events: Sender<ClientEvent>) -> Result<(), ()> {
    let _ = events.send(ClientEvent::Connect {
        addr,
        stream: stream.clone(),
    });

    let mut buf = [0; BUF_SIZE_512];
    loop {
        match stream.as_ref().read(&mut buf) {
            Ok(0) => {
                events
                    .send(ClientEvent::Disconnect { addr })
                    .expect("Send client disconnected");
                break;
            }
            Ok(n) => {
                let bytes = buf[..n].into();
                events
                    .send(ClientEvent::Read { addr, bytes })
                    .expect("Send new message");
            }
            Err(err) => {
                events
                    .send(ClientEvent::Error { addr, err })
                    .expect("Send client errored");
                break;
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), ()> {
    let address = format!("{}:{}", constants::ALL_HOSTS, constants::PORT);
    let listener = TcpListener::bind(&address).map_err(|err| {
        log_error!("Could not bing {}: {}", address, err);
    })?;
    log_info!("Started server at {address}");

    let (events_sender, events_receiver) = channel();
    thread::spawn(|| server(events_receiver));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => match stream.peer_addr() {
                Ok(client_addr) => {
                    let stream = Arc::new(stream);
                    let events_sender = events_sender.clone();
                    thread::spawn(move || client(stream, client_addr, events_sender));
                }
                Err(err) => log_error!("Could not get peer address: {}", err),
            },
            Err(err) => log_error!("Could not accept connection: {}", err),
        }
    }

    Ok(())
}

fn visible_map(map: &Arc<RwLock<ServerMap>>, coords: Coords, radius: u8) -> Vec<types::MapCell> {
    let map = map.read().unwrap();

    let radius = radius as u16;
    let radius_square = radius.pow(2);

    let top_left = (
        coords.0.checked_sub(radius).unwrap_or(0),
        coords.1.checked_sub(radius).unwrap_or(0),
    );
    let bottom_right = (
        min(1 + coords.0 + radius, map.height as u16),
        min(1 + coords.1 + radius, map.width as u16),
    );

    assert!(top_left.0 <= bottom_right.0);
    assert!(top_left.1 <= bottom_right.1);

    let mut res = vec![];
    for i in top_left.0..bottom_right.0 {
        for j in top_left.1..bottom_right.1 {
            let x = coords.0 as i16 - i as i16;
            let y = coords.1 as i16 - j as i16;

            // draw circle
            if x.pow(2) + y.pow(2) <= radius_square as i16 {
                res.push(types::MapCell {
                    block: map.coords[i as usize][j as usize].block,
                    coords: (i, j),
                });
            }
        }
    }

    res
}

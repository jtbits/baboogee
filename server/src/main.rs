use std::{
    cell::RefCell,
    collections::HashMap,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    ops::Deref,
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use game_core::{
    constants,
    protocol::{self, ClientPacket, Packet, Player},
    types::{Coords, Map},
    utils,
};
use logger::{log, log_error, log_info};
use proto_dryb::Deserialize;

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

pub struct Client {
    id: u32,
    conn: Arc<TcpStream>,
    coords: Coords,
    radius: u8,
}

impl Client {
    fn new_from_conn(conn: Arc<TcpStream>, max_x: i16, max_y: i16, id: &mut u32) -> Self {
        let coords = utils::generate_random_coords(max_x, max_y);
        let new = Self {
            conn,
            coords,
            radius: 5,
            id: *id,
        };
        *id += 1;
        new
    }

    fn write(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.conn.deref().write(buf)
    }
}

struct Server {
    clients: HashMap<SocketAddr, RefCell<Client>>,
    id_counter: u32,
    map: Map,
}

impl Server {
    fn new() -> Self {
        let map = utils::generate_map();
        Self {
            map,
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

        let client = Client::new_from_conn(
            stream,
            self.map.height as i16,
            self.map.width as i16,
            &mut self.id_counter,
        );

        let players_inside_radius = self
            .clients
            .values()
            .filter(|c| utils::is_inside_circle(client.coords, client.radius, c.borrow().coords))
            .map(|c| Player::new(c.borrow().id, c.borrow().coords))
            .collect::<Vec<_>>();

        let n = protocol::generate_initial_payload(
            buf,
            client.id,
            client.coords,
            client.radius,
            &self.map,
            players_inside_radius,
        )
        .map_err(|_| log_error!("Could not generate payload"))?;

        self.id_counter += 1;
        client
            .conn
            .deref()
            .write(&buf[..n])
            .map_err(|err| log_error!("Could not write to client: {addr}, {err}"))?;
        self.clients.insert(addr, RefCell::new(client));

        Ok(())
    }

    fn client_disconnected(&mut self, addr: SocketAddr, buf: &mut [u8]) -> Result<(), ()> {
        log_info!("Client {addr} disconnected");

        let removed = self
            .clients
            .remove(&addr)
            .ok_or(())
            .map_err(|_| log_error!("Did not found client in hashmap on disconnect"))?;

        let n = protocol::generate_player_disconnected(buf, removed.borrow().id)
            .map_err(|_| log_error!("Could not generate player_disconnected"))?;

        for c in self.clients.values() {
            let _ = c
                .borrow_mut()
                .write(&mut buf[..n])
                .inspect_err(|err| log_error!("Error writing to client: {err}"));
        }

        Ok(())
    }

    fn client_wrote(&mut self, addr: SocketAddr, bytes: &[u8], buf: &mut [u8]) -> Result<(), ()> {
        let client = self.clients.get(&addr).ok_or(()).map_err(|_| ())?;
        let (packet, _) = Packet::deserialize(bytes)
            .map_err(|_| log_error!("Could not deserialize packet from client"))?;

        match packet {
            Packet::Client(cp) => {
                match cp {
                    ClientPacket::Move(step) => {
                        log_info!("Got Move client packet with step: {:?}", step);
                        let (new_player_coord, new_visiple_coord) = utils::try_move_in_map(
                            &self.map,
                            client.borrow().coords,
                            step,
                            client.borrow().radius,
                            //self.clients.iter()
                            //.filter(|(_, c)| c.borrow().id != client.id)
                            //.map(|(_, c)| c.borrow().coords)
                            //.collect(),
                        )?;
                        let prev_player_coords = client.borrow().coords;
                        client.borrow_mut().coords = new_player_coord;
                        let players_inside_radius = self
                            .clients
                            .iter()
                            .filter(|(&a, _)| a != addr)
                            .filter(|(_, &ref c)| {
                                utils::is_inside_circle(
                                    new_player_coord,
                                    client.borrow().radius,
                                    c.borrow().coords,
                                )
                            })
                            .map(|(_, c)| Player::new(c.borrow().id, c.borrow().coords))
                            .collect();

                        // send new coords to player
                        let n = protocol::generate_new_coords_payload(
                            buf,
                            new_player_coord,
                            new_visiple_coord,
                            players_inside_radius,
                        )
                        .map_err(|_| ())?;
                        client
                            .borrow()
                            .conn
                            .deref()
                            .write(&buf[..n])
                            .map_err(|err| {
                                log_error!("Could not write to client: {addr}, {err}")
                            })?;

                        // send to other players new coords of this if in radius
                        let players_seeing_client = self
                            .clients
                            .iter()
                            .filter(|(&a, _)| a != addr)
                            .filter(|(_, c)| {
                                utils::is_inside_circle(
                                    c.borrow().coords,
                                    c.borrow().radius,
                                    new_player_coord,
                                )
                            });
                        for (&other_addr, other_client) in players_seeing_client {
                            log_info!(
                                "Sending move notification to player with id: {}",
                                client.borrow().id
                            );
                            let n = protocol::generate_move_notify_payload(
                                buf,
                                new_player_coord,
                                client.borrow().id,
                            )
                            .map_err(|_| ())?;
                            other_client
                                .borrow()
                                .conn
                                .deref()
                                .write(&buf[..n])
                                .map_err(|err| {
                                    log_error!(
                                    "Could not notify client {other_addr} about the move: {err}"
                                )
                                })?;
                        }

                        // sent to other players if player moved outside from their radius
                        let players_lost_client = self
                            .clients
                            .iter()
                            .filter(|(&a, _)| a != addr)
                            .filter(|(_, c)| {
                                utils::is_inside_circle(
                                    c.borrow().coords,
                                    c.borrow().radius,
                                    prev_player_coords,
                                )
                            })
                            .filter(|(_, c)| {
                                !utils::is_inside_circle(
                                    c.borrow().coords,
                                    c.borrow().radius,
                                    new_player_coord,
                                )
                            });

                        for (&other_addr, other_client) in players_lost_client {
                            log_info!(
                                "Sending move outside radius notification to player with id: {}",
                                client.borrow().id
                            );
                            let n = protocol::generate_move_outside_radius_notify_payload(
                                buf,
                                client.borrow().id,
                            )
                            .map_err(|_| ())?;
                            other_client
                                .borrow()
                                .conn
                                .deref()
                                .write(&buf[..n])
                                .map_err(|err| log_error!("Could not notify client {other_addr} about the move outside radius: {err}"))?;
                        }
                    }
                }
            }
            _ => return Err(()),
        }

        Ok(())
    }
}

fn server(events: Receiver<ClientEvent>) -> Result<(), ()> {
    let mut server = Server::new();
    let mut buf = [0; 512];

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

    let mut buf = [0; 256];
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

// TODO move somewhere else
#[cfg(test)]
mod tests {
    use proto_dryb::*;
    use proto_dryb_derive::*;

    #[test]
    fn testSimpleStruct() -> Result<(), SerializeError> {
        #[derive(Serialize)]
        struct Foo {
            x: u8,
        }

        let x = Foo { x: 1 };
        let mut buf = [0; 32];
        x.serialize(&mut buf)?;

        Ok(())
    }

    #[test]
    fn testStructWithEnum() -> Result<(), SerializeError> {
        #[derive(Serialize)]
        enum Bar {
            A,
            B,
        }

        #[derive(Serialize)]
        struct Foo {
            x: Bar,
            y: Bar,
            z: Bar,
        }

        let x = Foo {
            x: Bar::B,
            y: Bar::A,
            z: Bar::B,
        };
        let mut buf = [0; 32];
        x.serialize(&mut buf)?;

        Ok(())
    }

    #[test]
    fn testStructWithVec() -> Result<(), SerializeError> {
        #[derive(Serialize)]
        struct Foo {
            x: Vec<u8>,
        }

        let x = Foo {
            x: vec![1, 2, 3, 4, 5],
        };
        let mut buf = [0; 32];
        x.serialize(&mut buf)?;

        Ok(())
    }

    #[test]
    fn testEnumWithEnumAndStruct() -> Result<(), SerializeError> {
        #[derive(Serialize)]
        enum Foo {
            A(u8),
            B(u8, u8),
            C(u8, u8, u8),
            D,
        }

        let x = Foo::C(12, 34, 56);

        let mut buf = [0; 32];
        x.serialize(&mut buf)?;

        Ok(())
    }

    #[test]
    fn testStructWithVecOfTupples() -> Result<(), SerializeError> {
        #[derive(Serialize)]
        struct Foo {
            a: Vec<(u8, (u8, u8))>,
        }

        let x = Foo {
            a: vec![(1, (2, 3)), (4, (5, 6)), (7, (8, 9))],
        };

        let mut buf = [0; 32];
        x.serialize(&mut buf)?;

        Ok(())
    }
}

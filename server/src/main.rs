use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc,
    },
    thread,
    time::Duration, ops::Deref,
};

use game_core::{constants, protocol, types::{Map, Coords}, utils};
use logger::{log, log_error, log_info};

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
    coords: Coords,
}

impl Client {
    fn new_from_conn(conn: Arc<TcpStream>, max_x: i16, max_y: i16) -> Self {
        let coords = utils::generate_random_coords(max_x, max_y);
        Self { conn, coords }
    }
}

struct Server {
    clients: HashMap<SocketAddr, Client>,
    map: Map,
}

impl Server {
    fn new() -> Self {
        let map = utils::generate_map();
        Self {
            map,
            clients: HashMap::new(),
        }
    }

    fn client_connected(&mut self, buf: &mut [u8], addr: SocketAddr, stream: Arc<TcpStream>) {
        log_info!("Client {addr} connected");

        let client = Client::new_from_conn(stream, self.map.height as i16, self.map.width as i16);

        if let Ok(n) = protocol::generate_initial_payload(buf, client.coords, &self.map) {
            if let Err(err) = client.conn.deref().write(&buf[..n]) {
                log_error!("Could not write to client: {addr}, {err}");
                return;
            }
        } else {
            log_error!("Could not generate payload");
            return;
        }

        self.clients.insert(addr, client);
    }

    fn client_disconnected(&mut self, addr: SocketAddr) {
        self.clients.remove(&addr);

        log_info!("Client {addr} disconnected");
    }

    fn client_wrote(&self, addr: SocketAddr, bytes: &[u8]) {
        todo!()
    }
}

fn server(events: Receiver<ClientEvent>) -> Result<(), ()> {
    let mut server = Server::new();
    let mut buf = [0; 512];

    loop {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(msg) => match msg {
                ClientEvent::Connect { addr, stream } => {
                    server.client_connected(&mut buf, addr, stream)
                }
                ClientEvent::Disconnect { addr } => server.client_disconnected(addr),
                ClientEvent::Read { addr, bytes } => server.client_wrote(addr, &bytes),
                ClientEvent::Error { addr, err } => eprintln!("Client error: {}, {}", addr, err),
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

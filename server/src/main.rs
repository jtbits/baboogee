use std::{
    collections::HashMap,
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

const ALL_HOSTS: &'static str = "0.0.0.0";
const PORT: u16 = 42069;

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
}

impl Client {
    fn new_from_conn(conn: Arc<TcpStream>) -> Self {
        Self { conn }
    }
}

struct Server {
    clients: HashMap<SocketAddr, Client>,
}

impl Server {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    fn client_connected(&mut self, _buf: &mut [u8], addr: SocketAddr, stream: Arc<TcpStream>) {
        let client = Client::new_from_conn(stream);

        self.clients.insert(addr, client);

        println!("Client {addr} connected");
    }

    fn client_disconnected(&mut self, addr: SocketAddr) {
        self.clients.remove(&addr);

        println!("Client {addr} disconnected");
    }

    fn client_wrote(&self, addr: SocketAddr, bytes: &[u8]) {
        todo!()
    }
}

fn server(events: Receiver<ClientEvent>) -> Result<(), ()> {
    let mut server = Server::new();
    let mut buf = [0; 256];

    loop {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(msg) => match msg {
                ClientEvent::Connect { addr, stream } => server.client_connected(&mut buf, addr, stream),
                ClientEvent::Disconnect { addr } => server.client_disconnected(addr),
                ClientEvent::Read { addr, bytes } => server.client_wrote(addr, &bytes),
                ClientEvent::Error { addr, err } => eprintln!("Client error: {}, {}", addr, err),
            },
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => {
                eprintln!("Message receiver disconnected");
                return Err(());
            },
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
                println!("disc");
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
    let address = format!("{}:{}", ALL_HOSTS, PORT);
    let listener = TcpListener::bind(&address).map_err(|err| {
        eprintln!("Could not bing {}: {}", address, err);
    })?;
    println!("Started server at {address}");

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
                Err(err) => eprintln!("Could not get peer address: {}", err),
            },
            Err(err) => eprintln!("Could not accept connection: {}", err),
        }
    }

    Ok(())
}
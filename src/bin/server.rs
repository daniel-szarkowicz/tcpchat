use std::io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Result, Write};
use std::net::{
    IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs,
};
use std::time::{Duration, Instant};

use clap::Parser;
use log::{debug, info, trace};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    addr: IpAddr,
    #[arg(short, long, default_value_t = 6969)]
    port: u16,
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();
    let mut server = Server::new((args.addr, args.port))?;
    loop {
        server.update()?;
        if server.inactivity != 0 {
            let sleep_time = server.inactivity.min(25) * 10;
            trace!("Server is inactive, sleeping for {}ms", sleep_time);
            std::thread::sleep(Duration::from_millis(sleep_time));
        }
    }
}

struct Server {
    listener: TcpListener,
    clients: Vec<Client>,
    message_queue: Vec<String>,
    inactivity: u64,
}

impl Server {
    fn new<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let this = Self {
            listener,
            clients: Vec::default(),
            message_queue: Vec::default(),
            inactivity: 0,
        };
        info!(
            "Server started with address {}",
            this.listener.local_addr()?
        );
        Ok(this)
    }

    fn update(&mut self) -> Result<()> {
        let tick_start = Instant::now();
        self.inactivity += 1;
        trace!("Updating server");

        let listener_poll_start = Instant::now();
        self.poll_listener()?;
        let listener_poll_millis = listener_poll_start.elapsed().as_millis();

        let client_poll_start = Instant::now();
        self.message_queue
            .extend(self.clients.iter_mut().filter_map(Client::poll));
        let client_poll_millis = client_poll_start.elapsed().as_millis();

        let message_send_start = Instant::now();
        if !self.message_queue.is_empty() {
            let message_batch = self.message_queue.join("");
            self.inactivity = 0;
            for client in &mut self.clients {
                client.send(&message_batch);
                client.flush();
            }
            self.message_queue.clear();
        }
        let message_send_millis = message_send_start.elapsed().as_millis();

        let client_clear_start = Instant::now();
        let prev_clients_len = self.clients.len();
        self.clients.retain(Client::connected);
        if self.clients.len() != prev_clients_len {
            self.inactivity = 0;
        }
        let client_clear_millis = client_clear_start.elapsed().as_millis();

        let tick_millis = tick_start.elapsed().as_millis();
        log::log!(
            match tick_millis {
                20.. => log::Level::Warn,
                10.. => log::Level::Info,
                5.. => log::Level::Debug,
                _ => log::Level::Trace,
            },
            "Server tick took {}ms, (lp {}, cp {}, ms {}, cc {})",
            tick_millis,
            listener_poll_millis,
            client_poll_millis,
            message_send_millis,
            client_clear_millis,
        );

        Ok(())
    }

    fn poll_listener(&mut self) -> Result<bool> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                self.inactivity = 0;
                self.clients.push(Client::new(stream)?);
                Ok(true)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            // HACK: this error might not be fatal
            Err(e) => Err(e),
        }
    }
}

struct Client {
    addr: SocketAddr,
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
    stream: TcpStream,
    connected: bool,
}

impl Client {
    fn new(stream: TcpStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        let this = Self {
            addr: stream.peer_addr()?,
            reader: BufReader::new(stream.try_clone()?),
            writer: BufWriter::new(stream.try_clone()?),
            stream,
            connected: true,
        };
        info!("Client connected: {}", this.addr);
        Ok(this)
    }

    fn poll(&mut self) -> Option<String> {
        if !self.connected {
            return None;
        }
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(1..) => {
                debug!("Got message '{}' from {}", line.trim(), self.addr);
                Some(line)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => None,
            Err(e) => {
                self.disconnect(Some(e));
                None
            }
            Ok(0) => {
                self.disconnect(Some(Error::new(
                    ErrorKind::ConnectionAborted,
                    "Read 0 bytes from client",
                )));
                None
            }
        }
    }

    fn send(&mut self, message: &str) {
        if !self.connected {
            return;
        }
        trace!("Sending message '{}' to {}", message.trim(), self.addr);
        match self.writer.write_all(message.as_bytes()) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::WouldBlock => (),
            Err(e) => self.disconnect(Some(e)),
        }
    }

    fn flush(&mut self) {
        if !self.connected {
            return;
        }
        trace!("Flushing messages to {}", self.addr);
        match self.writer.flush() {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::WouldBlock => (),
            Err(e) => self.disconnect(Some(e)),
        }
    }

    fn disconnect(&mut self, reason: Option<Error>) {
        if !self.connected {
            return;
        }
        if let Some(e) = reason {
            info!(
                "Disconnecting client {}, error kind: {}, reason: {}",
                self.addr,
                e.kind(),
                e
            );
        } else {
            info!("Disconnecting client {}, no reason", self.addr);
        }
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        self.connected = false;
    }

    const fn connected(&self) -> bool {
        self.connected
    }
}

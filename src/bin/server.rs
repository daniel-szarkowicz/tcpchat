use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Result, Write};
use std::net::{
    IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs,
};
use std::time::Duration;

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
        self.inactivity += 1;
        trace!("Updating server");
        while self.poll_listener()? {}

        for client in &mut self.clients {
            self.message_queue
                .extend(std::iter::from_fn(|| client.poll()));
        }

        if !self.message_queue.is_empty() {
            self.inactivity = 0;
            for client in &mut self.clients {
                for message in &self.message_queue {
                    client.send(message);
                }
                client.flush();
            }
            self.message_queue.clear();
        }

        let prev_clients_len = self.clients.len();
        self.clients.retain(Client::connected);
        if self.clients.len() != prev_clients_len {
            self.inactivity = 0;
        }

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
            _ => {
                self.disconnect();
                None
            }
        }
    }

    fn send(&mut self, message: &str) {
        if !self.connected {
            return;
        }
        debug!("Sending message '{}' to {}", message.trim(), self.addr);
        if self.writer.write_all(message.as_bytes()).is_err() {
            self.disconnect();
        }
    }

    fn flush(&mut self) {
        if !self.connected {
            return;
        }
        trace!("Flushing messages to {}", self.addr);
        if self.writer.flush().is_err() {
            self.disconnect();
        }
    }

    fn disconnect(&mut self) {
        if !self.connected {
            return;
        }
        info!("Disconnecting client {}", self.addr);
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        self.connected = false;
    }

    const fn connected(&self) -> bool {
        self.connected
    }
}

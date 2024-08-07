use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Result, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

fn main() -> Result<()> {
    let mut server = Server::new("0.0.0.0:6969")?;
    loop {
        server.update()?;
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Server {
    listener: TcpListener,
    clients: Vec<Client>,
    message_queue: Vec<String>,
}

impl Server {
    fn new<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            clients: Vec::default(),
            message_queue: Vec::default(),
        })
    }

    fn update(&mut self) -> Result<()> {
        while self.poll_listener()? {}

        for client in &mut self.clients {
            self.message_queue
                .extend(std::iter::from_fn(|| client.poll()));
        }

        for client in &mut self.clients {
            for message in &self.message_queue {
                client.send(message);
            }
            client.flush();
        }
        self.message_queue.clear();

        self.clients.retain(Client::connected);

        Ok(())
    }

    fn poll_listener(&mut self) -> Result<bool> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                self.clients.push(Client::new(stream)?);
                Ok(true)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e),
        }
    }
}

struct Client {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
    connected: bool,
}

impl Client {
    fn new(stream: TcpStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            reader: BufReader::new(stream.try_clone()?),
            writer: BufWriter::new(stream),
            connected: true,
        })
    }

    fn poll(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(1..) => Some(line),
            Err(e) if e.kind() == ErrorKind::WouldBlock => None,
            _ => {
                self.connected = false;
                None
            }
        }
    }

    fn send(&mut self, message: &str) {
        if self.writer.write_all(message.as_bytes()).is_err() {
            self.connected = false;
        }
    }

    fn flush(&mut self) {
        if self.writer.flush().is_err() {
            self.connected = false;
        }
    }

    const fn connected(&self) -> bool {
        self.connected
    }
}

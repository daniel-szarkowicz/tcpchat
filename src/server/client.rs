use std::io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Result, Write};
use std::net::{SocketAddr, TcpStream};

use log::{debug, info, trace};

#[derive(Debug)]
pub struct Client {
    addr: SocketAddr,
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
    stream: TcpStream,
    connected: bool,
}

impl Client {
    pub fn new(stream: TcpStream) -> Result<Self> {
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

    pub fn poll(&mut self) -> Option<String> {
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

    pub fn send(&mut self, message: &str) {
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

    pub fn flush(&mut self) {
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

    #[must_use]
    pub const fn connected(&self) -> bool {
        self.connected
    }
}

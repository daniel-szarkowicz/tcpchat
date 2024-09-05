use std::io::{Error, ErrorKind, Result, Write};
use std::net::{SocketAddr, TcpStream};

use log::{debug, info, trace};

use crate::common::commands::{ClientCommand, ServerCommand};
use crate::common::Connection;

#[derive(Debug)]
pub struct Server {
    addr: SocketAddr,
    connection: Connection<ClientCommand, ServerCommand>,
    connected: bool,
}

impl Server {
    pub fn new(stream: TcpStream) -> Result<Self> {
        let this = Self {
            addr: stream.peer_addr()?,
            connection: Connection::new(stream)?,
            connected: true,
        };
        info!("Server connected: {}", this.addr);
        Ok(this)
    }

    pub fn poll(&mut self) -> Option<ServerCommand> {
        if !self.connected {
            return None;
        }
        match self.connection.receive() {
            Ok(msg) => {
                debug!("Got message '{:?}' from {}", msg, self.addr);
                Some(msg)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => None,
            Err(e) => {
                self.disconnect(Some(e));
                None
            }
        }
    }

    pub fn send(&mut self, message: &ClientCommand) {
        if !self.connected {
            return;
        }
        trace!("Sending message '{:?}' to {}", message, self.addr);
        match self.connection.send(message) {
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
        match self.connection.flush() {
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
                "Disconnecting server {}, error kind: {}, reason: {}",
                self.addr,
                e.kind(),
                e
            );
        } else {
            info!("Disconnecting server {}, no reason", self.addr);
        }
        let _ = self.connection.shutdown(std::net::Shutdown::Both);
        self.connected = false;
    }

    #[must_use]
    pub const fn connected(&self) -> bool {
        self.connected
    }
}

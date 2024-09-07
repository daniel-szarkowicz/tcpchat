use std::io::{Error, ErrorKind, Result, Write};
use std::net::{SocketAddr, TcpStream};

use log::{debug, info, trace};

use common::commands::{ClientCommand, ServerCommand};
use common::Connection;

#[derive(Debug)]
pub struct Client {
    addr: SocketAddr,
    connection: Connection<ServerCommand, ClientCommand>,
    connected: bool,
    user_id: u16,
}

impl Client {
    pub fn new(stream: TcpStream, user_id: u16) -> Result<Self> {
        let this = Self {
            addr: stream.peer_addr()?,
            connection: Connection::new(stream)?,
            connected: true,
            user_id,
        };
        info!("Client connected: {}", this.addr);
        Ok(this)
    }

    pub fn poll(&mut self) -> Option<ClientCommand> {
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

    pub fn send(&mut self, message: &ServerCommand) {
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
                "Disconnecting client {}, error kind: {}, reason: {}",
                self.addr,
                e.kind(),
                e
            );
        } else {
            info!("Disconnecting client {}, no reason", self.addr);
        }
        let _ = self.connection.shutdown(std::net::Shutdown::Both);
        self.connected = false;
    }

    #[must_use]
    pub const fn connected(&self) -> bool {
        self.connected
    }

    #[must_use]
    pub const fn user_id(&self) -> u16 {
        self.user_id
    }
}

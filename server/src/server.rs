use std::io::{ErrorKind, Result};
use std::net::{TcpListener, ToSocketAddrs};
use std::time::Instant;

use log::{info, trace};

use crate::Client;
use common::commands::{ClientCommand, ServerCommand};

#[derive(Debug)]
struct IdGen {
    id: u16,
}

impl IdGen {
    fn new() -> Self {
        Self { id: 0 }
    }

    fn get(&mut self) -> u16 {
        self.id += 1;
        self.id
    }
}

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    clients: Vec<Client>,
    message_queue: Vec<ServerCommand>,
    pub inactivity: u64,
    user_id_gen: IdGen,
    msg_id_gen: IdGen,
}

impl Server {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let this = Self {
            listener,
            clients: Vec::default(),
            message_queue: Vec::default(),
            inactivity: 0,
            user_id_gen: IdGen::new(),
            msg_id_gen: IdGen::new(),
        };
        info!(
            "Server started with address {}",
            this.listener.local_addr()?
        );
        Ok(this)
    }

    pub fn update(&mut self) -> Result<()> {
        let tick_start = Instant::now();
        self.inactivity += 1;
        trace!("Updating server");

        let listener_poll_start = Instant::now();
        self.poll_listener()?;
        let listener_poll_elapsed = listener_poll_start.elapsed();

        let client_poll_start = Instant::now();
        self.message_queue.extend(
            self.clients
                .iter_mut()
                .filter_map(|c| c.poll().map(|cc| (c, cc)))
                .filter_map(|(c, cc)| match cc {
                    ClientCommand::Padding => None,
                    ClientCommand::Connect { name } => {
                        let user_id = c.user_id();
                        Some(ServerCommand::AddUser { user_id, name })
                    }
                    ClientCommand::Message { message } => {
                        Some(ServerCommand::Message {
                            msg_id: self.msg_id_gen.get(),
                            user_id: c.user_id(),
                            message,
                        })
                    }
                }),
        );
        let client_poll_elapsed = client_poll_start.elapsed();

        let message_send_start = Instant::now();
        // if !self.message_queue.is_empty() {
        for message in &self.message_queue {
            self.inactivity = 0;
            for client in &mut self.clients {
                client.send(message);
                client.flush();
            }
        }
        self.message_queue.clear();
        // }
        let message_send_elapsed = message_send_start.elapsed();

        let client_clear_start = Instant::now();
        let prev_clients_len = self.clients.len();
        self.clients.retain(|c| {
            if c.connected() {
                true
            } else {
                self.message_queue.push(ServerCommand::RemoveUser {
                    user_id: c.user_id(),
                });
                false
            }
        });
        if self.clients.len() != prev_clients_len {
            self.inactivity = 0;
        }
        let client_clear_elapsed = client_clear_start.elapsed();

        let tick_elapsed = tick_start.elapsed();
        log::log!(
            match tick_elapsed.as_micros() {
                100_000.. => log::Level::Warn,
                10000.. => log::Level::Info,
                1000.. => log::Level::Debug,
                _ => log::Level::Trace,
            },
            "Server tick took {}us, (lp {}, cp {}, ms {}, cc {})",
            tick_elapsed.as_micros(),
            listener_poll_elapsed.as_micros(),
            client_poll_elapsed.as_micros(),
            message_send_elapsed.as_micros(),
            client_clear_elapsed.as_micros(),
        );

        Ok(())
    }

    fn poll_listener(&mut self) -> Result<bool> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                self.inactivity = 0;
                self.clients
                    .push(Client::new(stream, self.user_id_gen.get())?);
                Ok(true)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            // HACK: this error might not be fatal
            Err(e) => Err(e),
        }
    }
}

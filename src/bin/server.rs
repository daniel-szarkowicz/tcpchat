use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Result;

enum Message {
    Text(String),
}

struct Client {
    _addr: SocketAddr,
    sender: Sender<Message>,
    _handle: JoinHandle<()>,
    id: usize,
}

fn main() -> Result<()> {
    println!("Server started!");
    let listener = TcpListener::bind("0.0.0.0:6969")?;

    let (server_sender, server_receiver) = channel();

    let mut clients = vec![];
    let mut client_id = 0;

    listener.set_nonblocking(true)?;

    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                println!("Handling client: {_addr}");
                let (client_sender, client_receiver) = channel();
                client_id += 1;
                let client = Client {
                    _addr,
                    sender: client_sender,
                    _handle: handle_client(
                        stream,
                        server_sender.clone(),
                        client_receiver,
                        client_id,
                    ),
                    id: client_id,
                };
                clients.push(client);
            }
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock {
                    Err(e)?
                }
            }
        }
        match server_receiver.try_recv() {
            Ok(Message::Text(t)) => {
                for client in &clients {
                    client.sender.send(Message::Text(t.clone()))?;
                }
            }
            Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected)?,
            Err(TryRecvError::Empty) => {}
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn handle_client(
    stream: TcpStream,
    server_sender: Sender<Message>,
    client_receiver: Receiver<Message>,
    client_id: usize,
) -> JoinHandle<()> {
    let thread = move || -> Result<()> {
        stream.set_nonblocking(true)?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut writer = BufWriter::new(stream);
        writeln!(writer, "Hello from the server!")?;
        loop {
            writer.flush()?;
            while {
                let mut line = format!("#{client_id}: ");
                match reader.read_line(&mut line) {
                    Ok(0) => return Ok(()),
                    Ok(_) => {
                        println!("read line: '{}'", line);
                        server_sender.send(Message::Text(line))?;
                        true
                    }
                    Err(e) => {
                        if e.kind() != ErrorKind::WouldBlock {
                            Err(e)?;
                        }
                        false
                    }
                }
            } {}
            if let Ok(Message::Text(message)) = client_receiver.try_recv() {
                println!("sending message: '{}'", message);
                write!(writer, "{}", message)?;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    std::thread::spawn(move || {
        if let Err(e) = thread() {
            println!("Client exited with error: {}", e);
        } else {
            println!("Client exited");
        }
    })
}

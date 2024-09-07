use std::io::Result;
use std::net::TcpStream;
use std::time::Duration;

use common::commands::ClientCommand;
use log::{error, info};

use client::ui::{UI, UIEvent};

use client::channel_logger;
use client::Server;


fn main() -> Result<()> {
    let log_receiver = channel_logger::init_and_get_receiver();
    let mut ui = UI::new()?;
    let mut run = true;
    let mut server = None::<Server>;

    while run {
        if let Some(server) = &mut server {
            while let Some(msg) = server.poll() {
                ui.add_message(msg);
            }
        }
        while let Ok(log) = log_receiver.try_recv() {
            ui.add_log(log);
        }
        while let Some(event) = ui.poll()? {
            match event {
                UIEvent::Exit => run = false,
                UIEvent::Message(msg) => {
                    if let Some(server) = &mut server {
                        server.send(&ClientCommand::Message { message: msg });
                        server.flush();
                    } else {
                        error!("Server not connected!");
                        info!(
                            "Use `/connect <address> <username>` to connect."
                        );
                    }
                }
                UIEvent::Connect {
                    server_addr,
                    user_name,
                } => {
                    server = TcpStream::connect(server_addr)
                        .inspect_err(|e| {
                            error!("Failed to connect to the server: {e}")
                        })
                        .ok()
                        .and_then(|s| Server::new(s).inspect_err(|e| 
                            error!("Failed to connect to the server: {e}")
                        ).ok())
                        .map(|mut s| {
                            s.send(&ClientCommand::Connect {
                                name: user_name,
                            });
                            s
                        })
                }
                UIEvent::Disconnect => {
                    server = None
                }
            }
        }
        ui.render()?;
        if let Some(s) = &mut server {
            if !s.connected() {
                server = None;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}


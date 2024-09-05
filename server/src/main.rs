use std::io::Result;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use clap::Parser;
use log::trace;

use server::Server;

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

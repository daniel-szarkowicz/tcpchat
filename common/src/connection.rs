use std::io::{Cursor, Error, ErrorKind, Read, Result, Write};
use std::marker::PhantomData;
use std::net::TcpStream;
use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

use super::Codec;

const READ_TIMEOUT: Duration = Duration::from_millis(1000);

#[derive(Debug)]
pub struct Connection<Sent: Codec + ?Sized, Received: Codec + ?Sized> {
    stream: TcpStream,
    buffer: Vec<u8>,
    _sent: PhantomData<Sent>,
    _received: PhantomData<Received>,
    timeout: Option<Instant>,
}

impl<Sent: Codec + ?Sized, Received: Codec + ?Sized>
    Connection<Sent, Received>
{
    pub fn new(stream: TcpStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            buffer: vec![],
            _sent: PhantomData,
            _received: PhantomData,
            timeout: None,
        })
    }

    pub fn receive(&mut self) -> Result<Received::Owned> {
        if self.buffer.is_empty() {
            let mut buf = [0; size_of::<u16>()];
            read_exact(&mut self.stream, &mut buf, &mut self.timeout)?;
            let data_size = u16::from_be_bytes(buf);
            self.buffer.resize(data_size as usize, 0);
        }
        debug_assert!(!self.buffer.is_empty());
        read_exact(&mut self.stream, &mut self.buffer, &mut self.timeout)?;
        Received::decode(&mut Cursor::new(std::mem::take(&mut self.buffer)))
    }

    pub fn send(&mut self, msg: &Sent) -> Result<()> {
        let data_size = msg.coded_size() as u16;
        self.stream.write_all(&data_size.to_be_bytes())?;
        msg.code(&mut self.stream)
    }
}

// TODO: move into `Connection`
fn read_exact(
    stream: &mut TcpStream,
    buf: &mut [u8],
    timeout: &mut Option<Instant>,
) -> Result<()> {
    match stream.peek(buf)? {
        n if n == buf.len() => stream.read_exact(buf),
        0 => Err(Error::new(
            ErrorKind::ConnectionAborted,
            "read 0 bytes from stream",
        )),
        _ => {
            let timeout = timeout.get_or_insert_with(Instant::now);
            if timeout.elapsed() > READ_TIMEOUT {
                Err(Error::from(ErrorKind::TimedOut))
            } else {
                Err(Error::from(ErrorKind::WouldBlock))
            }
        }
    }
}

impl<Sent: Codec + ?Sized, Received: Codec + ?Sized> Deref
    for Connection<Sent, Received>
{
    type Target = TcpStream;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<Sent: Codec + ?Sized, Received: Codec + ?Sized> DerefMut
    for Connection<Sent, Received>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

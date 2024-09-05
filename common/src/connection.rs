use std::io::{Cursor, Error, ErrorKind, Read, Result, Write};
use std::marker::PhantomData;
use std::net::TcpStream;
use std::ops::{Deref, DerefMut};

use super::Codec;

#[derive(Debug)]
pub struct Connection<Sent: Codec + ?Sized, Received: Codec + ?Sized> {
    stream: TcpStream,
    _sent: PhantomData<Sent>,
    _received: PhantomData<Received>,

    read_buffer: Vec<u8>,
    read_buffer_actual: usize,
    read_mode: ReadMode,
}

#[derive(Debug)]
enum ReadMode {
    Size,
    Data,
}

impl<Sent: Codec + ?Sized, Received: Codec + ?Sized>
    Connection<Sent, Received>
{
    pub fn new(stream: TcpStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            _sent: PhantomData,
            _received: PhantomData,

            read_buffer: vec![0; size_of::<u16>()],
            read_buffer_actual: 0,
            read_mode: ReadMode::Size,
        })
    }

    pub fn receive(&mut self) -> Result<Received::Owned> {
        let n = self
            .stream
            .read(&mut self.read_buffer[self.read_buffer_actual..])?;
        if n == 0 {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }
        self.read_buffer_actual += n;
        if self.read_buffer_actual == self.read_buffer.len() {
            match self.read_mode {
                ReadMode::Size => {
                    let data_size =
                        u16::decode(&mut Cursor::new(&mut self.read_buffer))
                            .unwrap_or_else(|_| unreachable!());
                    self.read_buffer.resize(data_size as usize, 0);
                    self.read_buffer_actual = 0;
                    self.read_mode = ReadMode::Data;
                    self.receive()
                }
                ReadMode::Data => {
                    let result = Received::decode(&mut Cursor::new(
                        &mut self.read_buffer,
                    ));
                    self.read_buffer.resize(2, 0);
                    self.read_buffer_actual = 0;
                    self.read_mode = ReadMode::Size;
                    result
                }
            }
        } else {
            Err(Error::from(ErrorKind::WouldBlock))
        }
    }

    pub fn send(&mut self, msg: &Sent) -> Result<()> {
        let data_size = msg.coded_size() as u16;
        self.stream.write_all(&data_size.to_be_bytes())?;
        msg.code(&mut self.stream)
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

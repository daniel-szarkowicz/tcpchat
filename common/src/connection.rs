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

    read_buffer: Cursor<Vec<u8>>,
    read_buffer_actual: usize,
    read_mode: ReadMode,
}

type DataSize = u16;

#[derive(Debug)]
enum ReadMode {
    Size,
    Data,
    Parse,
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

            read_buffer: Cursor::new(vec![]),
            read_buffer_actual: 0,
            read_mode: ReadMode::Parse,
        })
    }

    pub fn receive(&mut self) -> Result<Received::Owned> {
        loop {
            match self.read_mode {
                ReadMode::Size => {
                    self.fill_buf()?;
                    let data_size = DataSize::decode(&mut self.read_buffer)
                        .unwrap_or_else(|_| {
                            unreachable!(
                                "len: {}\npos: {}",
                                self.read_buffer.get_ref().len(),
                                self.read_buffer.position(),
                            )
                        });
                    self.resize_buf(data_size as usize);
                    self.read_mode = ReadMode::Data;
                }
                ReadMode::Data => {
                    self.fill_buf()?;
                    self.read_mode = ReadMode::Parse;
                }
                ReadMode::Parse => {
                    if (self.read_buffer.position() as usize)
                        < self.read_buffer.get_ref().len()
                    {
                        return Received::decode(&mut self.read_buffer);
                    }
                    self.resize_buf(size_of::<DataSize>());
                    self.read_mode = ReadMode::Size;
                }
            }
        }
    }

    fn resize_buf(&mut self, size: usize) {
        self.read_buffer.get_mut().resize(size, 0);
        self.read_buffer.set_position(0);
        self.read_buffer_actual = 0;
    }

    fn fill_buf(&mut self) -> Result<()> {
        let n = self
            .stream
            .read(&mut self.read_buffer.get_mut()[self.read_buffer_actual..])?;
        if n == 0 {
            Err(Error::from(ErrorKind::UnexpectedEof))
        } else {
            self.read_buffer_actual += n;
            if self.read_buffer_actual == self.read_buffer.get_ref().len() {
                Ok(())
            } else {
                Err(Error::from(ErrorKind::WouldBlock))
            }
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

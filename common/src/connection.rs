use std::io::{Result, Write};
use std::marker::PhantomData;
use std::net::TcpStream;
use std::ops::{Deref, DerefMut};

use crate::Buffer;

use super::Codec;

#[derive(Debug)]
pub struct Connection<Sent: Codec + ?Sized, Received: Codec + ?Sized> {
    stream: TcpStream,
    _sent: PhantomData<Sent>,
    _received: PhantomData<Received>,

    buffer: Buffer,
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

            buffer: Buffer::new(),
            read_mode: ReadMode::Parse,
        })
    }

    pub fn receive(&mut self) -> Result<Received::Owned> {
        loop {
            match self.read_mode {
                ReadMode::Size => {
                    self.buffer.try_fill_from(&mut self.stream)?;
                    let data_size = DataSize::decode(&mut self.buffer)
                        .unwrap_or_else(|_| unreachable!());
                    self.buffer.resize(data_size as usize);
                    self.read_mode = ReadMode::Data;
                }
                ReadMode::Data => {
                    self.buffer.try_fill_from(&mut self.stream)?;
                    self.read_mode = ReadMode::Parse;
                }
                ReadMode::Parse => {
                    if !self.buffer.finished() {
                        return Received::decode(&mut self.buffer);
                    }
                    self.buffer.resize(size_of::<DataSize>());
                    self.read_mode = ReadMode::Size;
                }
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

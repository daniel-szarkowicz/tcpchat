use core::str;
use std::io::{Error, ErrorKind, Read, Result, Write};

use log::debug;

pub trait Codec: ToOwned {
    fn code(&self, w: &mut impl Write) -> Result<()>;
    fn decode(r: &mut impl Read) -> Result<Self::Owned>;
    fn coded_size(&self) -> usize;
}

impl Codec for str {
    fn code(&self, w: &mut impl Write) -> Result<()> {
        self.as_bytes().code(w)
    }

    fn decode(r: &mut impl Read) -> Result<Self::Owned> {
        type Slice = [u8];
        String::from_utf8(Slice::decode(r)?)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))
    }

    fn coded_size(&self) -> usize {
        self.as_bytes().coded_size()
    }
}

#[allow(clippy::cast_possible_truncation)]
impl Codec for [u8] {
    fn code(&self, w: &mut impl Write) -> Result<()> {
        (self.len() as u16).code(w)?;
        w.write_all(self)
    }

    fn decode(r: &mut impl Read) -> Result<Self::Owned> {
        let mut data_buf = vec![0; u16::decode(r)? as usize];
        debug!("data_buf len = {}", data_buf.len());
        r.read_exact(&mut data_buf)?;
        Ok(data_buf)
    }

    fn coded_size(&self) -> usize {
        (self.len() as u16).coded_size() + self.len()
    }
}

impl Codec for u16 {
    fn code(&self, w: &mut impl Write) -> Result<()> {
        let buf = self.to_be_bytes();
        w.write_all(&buf)
    }

    fn decode(r: &mut impl Read) -> Result<Self::Owned> {
        let mut buf = [0; 2];
        r.read_exact(&mut buf)?;
        Ok(Self::from_be_bytes(buf))
    }

    fn coded_size(&self) -> usize {
        size_of::<Self>()
    }
}

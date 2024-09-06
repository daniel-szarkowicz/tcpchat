use std::io::{Error, ErrorKind, Read, Result};

#[derive(Debug)]
pub struct Buffer {
    buf: Vec<u8>,
    cursor: usize,
}

impl Read for Buffer {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let read_buf = &self.buf[self.cursor..];
        let copy_len = buf.len().min(read_buf.len());
        buf[..copy_len].copy_from_slice(&read_buf[..copy_len]);
        self.cursor += copy_len;
        Ok(copy_len)
    }
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            buf: vec![],
            cursor: 0,
        }
    }

    pub fn try_fill_from(&mut self, r: &mut impl Read) -> Result<()> {
        let n = r.read(&mut self.buf[self.cursor..])?;
        if n == 0 {
            Err(Error::from(ErrorKind::UnexpectedEof))
        } else {
            self.cursor += n;
            if self.finished() {
                self.cursor = 0;
                Ok(())
            } else {
                Err(Error::from(ErrorKind::WouldBlock))
            }
        }
    }

    pub fn resize(&mut self, size: usize) {
        self.buf.resize(size, 0);
        self.cursor = 0;
    }

    pub fn finished(&self) -> bool {
        self.cursor == self.buf.len()
    }
}

use std::io::{Error, ErrorKind, Result, Write};

use super::Codec;

#[derive(Debug, Clone)]
pub enum ClientCommand {
    Padding,
    Connect { name: String },
    Message { message: String },
}

#[derive(Debug, Clone)]
pub enum ServerCommand {
    Padding,
    AddUser {
        user_id: u16,
        name: String,
    },
    RemoveUser {
        user_id: u16,
    },
    Message {
        msg_id: u16,
        user_id: u16,
        message: String,
    },
}

impl Codec for ClientCommand {
    fn code(&self, w: &mut impl Write) -> Result<()> {
        match self {
            Self::Padding => 0u16.code(w),
            Self::Connect { name } => {
                1u16.code(w)?;
                name.code(w)
            }
            Self::Message { message } => {
                2u16.code(w)?;
                message.code(w)
            }
        }
    }

    fn decode(r: &mut impl std::io::Read) -> Result<Self::Owned> {
        let id = u16::decode(r)?;
        Ok(match id {
            0 => Self::Padding,
            1 => Self::Connect {
                name: str::decode(r)?,
            },
            2 => Self::Message {
                message: str::decode(r)?,
            },
            _ => return Err(Error::from(ErrorKind::InvalidData)),
        })
    }

    fn coded_size(&self) -> usize {
        match self {
            Self::Padding => 0u16.coded_size(),
            Self::Connect { name } => 1u16.coded_size() + name.coded_size(),
            Self::Message { message } => {
                2u16.coded_size() + message.coded_size()
            }
        }
    }
}

impl Codec for ServerCommand {
    fn code(&self, w: &mut impl Write) -> Result<()> {
        match self {
            Self::Padding => 0u16.code(w),
            Self::AddUser { user_id, name } => {
                1u16.code(w)?;
                user_id.code(w)?;
                name.code(w)
            }
            Self::RemoveUser { user_id } => {
                2u16.code(w)?;
                user_id.code(w)
            }
            Self::Message {
                msg_id,
                user_id,
                message,
            } => {
                3u16.code(w)?;
                msg_id.code(w)?;
                user_id.code(w)?;
                message.code(w)
            }
        }
    }

    fn decode(r: &mut impl std::io::Read) -> Result<Self::Owned> {
        let id = u16::decode(r)?;
        Ok(match id {
            0 => Self::Padding,
            1 => Self::AddUser {
                user_id: u16::decode(r)?,
                name: str::decode(r)?,
            },
            2 => Self::RemoveUser {
                user_id: u16::decode(r)?,
            },
            3 => Self::Message {
                msg_id: u16::decode(r)?,
                user_id: u16::decode(r)?,
                message: str::decode(r)?,
            },
            _ => return Err(Error::from(ErrorKind::InvalidData)),
        })
    }

    fn coded_size(&self) -> usize {
        match self {
            Self::Padding => 0u16.coded_size(),
            Self::AddUser { user_id, name } => {
                1u16.coded_size() + user_id.coded_size() + name.coded_size()
            }
            Self::RemoveUser { user_id } => {
                2u16.coded_size() + user_id.coded_size()
            }
            Self::Message {
                msg_id,
                user_id,
                message,
            } => {
                3u16.coded_size()
                    + msg_id.coded_size()
                    + user_id.coded_size()
                    + message.coded_size()
            }
        }
    }
}

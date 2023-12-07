use std::{error::Error, fmt};

#[derive(Debug)]
pub enum SerializeError {
    BufferOverflow,
}

impl fmt::Display for SerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerializeError::BufferOverflow => write!(f, "Buffer overflow"),
        }
    }
}

impl Error for SerializeError {}

pub trait Serialize {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError>;
}

impl<T> Serialize for Option<T>
where
    T: Serialize,
{
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        match self {
            Some(x) => {
                buffer[0] = 1;
                Ok(x.serialize(&mut buffer[1..])? + 1)
            }
            None => {
                buffer[0] = 0;
                Ok(1)
            }
        }
    }
}

impl<T> Serialize for Vec<T>
where
    T: Serialize,
{
    fn serialize(&self, buf: &mut [u8]) -> Result<usize, SerializeError> {
        if buf.len() < 1 {
            return Err(SerializeError::BufferOverflow);
        }

        let len = self.len() as u8;
        let _ = len.serialize(&mut buf[..])?;

        let mut offset = 1;
        for item in self {
            let used = item.serialize(&mut buf[offset..])?;
            offset += used;
        }

        Ok(offset)
    }
}

// Primitive implimintations
impl Serialize for u8 {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        if buffer.len() < 1 {
            return Err(SerializeError::BufferOverflow);
        }

        buffer[0] = *self;

        Ok(1)
    }
}

impl Serialize for i8 {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        if buffer.len() < 1 {
            return Err(SerializeError::BufferOverflow);
        }

        buffer[0] = *self as u8;

        Ok(1)
    }
}

impl Serialize for u16 {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        if buffer.len() < 2 {
            return Err(SerializeError::BufferOverflow);
        }

        buffer[0] = (*self >> 8) as u8;
        buffer[1] = *self as u8;

        Ok(2)
    }
}

impl Serialize for i16 {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        if buffer.len() < 2 {
            return Err(SerializeError::BufferOverflow);
        }

        buffer[0] = (*self >> 8) as u8;
        buffer[1] = *self as u8;

        Ok(2)
    }
}

// TODO u32 i32 u64 i64

#[derive(Debug)]
pub enum DeserializeError {
    Invalid,
}

impl fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeserializeError::Invalid => write!(f, "Invalid payload"),
        }
    }
}

impl Error for DeserializeError {}

pub trait Deserialize: Sized {
    fn deserialize(buffer: &[u8]) -> Result<Self, DeserializeError>;
}

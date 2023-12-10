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
        if buffer.len() < 1 {
            return Err(SerializeError::BufferOverflow);
        }

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

macro_rules! impl_serialize_tuple{
    ($($idx:tt $t:tt),+) => {
        impl<$($t,)+> Serialize for ($($t,)+)
            where
                $($t: Serialize,)+
                {
                    fn serialize(&self, buf: &mut [u8]) -> Result<usize, SerializeError> {
                        let mut offset = 0;

                        $(
                            offset += self.$idx.serialize(&mut buf[offset..])?;
                         )+

                            Ok(offset)
                    }
                }
    };
}

impl_serialize_tuple!(0 A, 1 B);
// TODO add tuples of size 3,4..N when needed

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

impl Serialize for u32 {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        if buffer.len() < 4 {
            return Err(SerializeError::BufferOverflow);
        }

        buffer[0] = (*self >> 24) as u8;
        buffer[1] = (*self >> 16) as u8;
        buffer[2] = (*self >> 8) as u8;
        buffer[3] = *self as u8;

        Ok(4)
    }
}

impl Serialize for i32 {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        if buffer.len() < 4 {
            return Err(SerializeError::BufferOverflow);
        }

        buffer[0] = (*self >> 24) as u8;
        buffer[1] = (*self >> 16) as u8;
        buffer[2] = (*self >> 8) as u8;
        buffer[3] = *self as u8;

        Ok(4)
    }
}

// TODO u64 i64

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
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError>;
}

impl<T> Deserialize for Option<T>
where
    T: Deserialize,
{
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 1 {
            return Err(DeserializeError::Invalid);
        }

        match buf[0] {
            0 => Ok((None, 1)),
            1 => {
                let (val, size) = T::deserialize(&buf[1..])?;
                Ok((Some(val), size + 1))
            }
            _ => Err(DeserializeError::Invalid),
        }
    }
}

impl<T> Deserialize for Vec<T>
where
    T: Deserialize,
{
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 1 {
            return Err(DeserializeError::Invalid);
        }

        let len = buf[0] as usize;
        let mut v = Vec::with_capacity(len);

        let mut offset = 1;
        for _ in 0..len {
            let (val, size) = T::deserialize(&buf[offset..])?;
            v.push(val);
            offset += size;
        }

        Ok((v, offset))
    }
}

macro_rules! impl_deserialize_tuple{
    ($($idx:tt $t:tt $n:tt),+) => {
        impl<$($t,)+> Deserialize for ($($t,)+)
            where
                $($t: Deserialize,)+
                {
                    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
                        let mut offset = 0;

                        $(
                            let ($n, size) = $t::deserialize(&buf[offset..])?;
                            offset += size;
                         )+

                        Ok((($($n,)+), offset))
                    }
                }
    };
}

impl_deserialize_tuple!(0 A a, 1 B b);
// TODO add tuples of size 3,4..N when needed

// Primitive implimintations
impl Deserialize for u8 {
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 1 {
            return Err(DeserializeError::Invalid);
        }

        Ok((buf[0], 1))
    }
}

impl Deserialize for i8 {
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 1 {
            return Err(DeserializeError::Invalid);
        }

        Ok((buf[0] as i8, 1))
    }
}

impl Deserialize for u16 {
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 1 {
            return Err(DeserializeError::Invalid);
        }

        let x1 = buf[0] as u16;
        let x2 = buf[1] as u16;

        Ok(((x1 << 8) | (x2 & 0xff), 2))
    }
}

impl Deserialize for i16 {
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 1 {
            return Err(DeserializeError::Invalid);
        }

        let x1 = buf[0] as i16;
        let x2 = buf[1] as i16;

        Ok(((x1 << 8) | (x2 & 0xff), 2))
    }
}

impl Deserialize for u32 {
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 4 {
            return Err(DeserializeError::Invalid);
        }

        let x1 = buf[0] as u32;
        let x2 = buf[1] as u32;
        let x3 = buf[2] as u32;
        let x4 = buf[3] as u32;

        Ok((((x1 << 24) | (x2 << 16) | (x3 << 8) | x4), 4))
    }
}

impl Deserialize for i32 {
    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
        if buf.len() < 4 {
            return Err(DeserializeError::Invalid);
        }

        let x1 = buf[0] as i32;
        let x2 = buf[1] as i32;
        let x3 = buf[2] as i32;
        let x4 = buf[3] as i32;

        Ok((((x1 << 24) | (x2 << 16) | (x3 << 8) | x4), 4))
    }
}

// TODO u64 i64

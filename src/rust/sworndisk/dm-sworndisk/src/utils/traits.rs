use crate::prelude::*;

/// Serailize trait: convert a struct into binary bufferr (Vec<u8>)
pub trait Serialize {
    fn serialize(&self) -> Result<Vec<u8>>;
}

/// Deserialize trait: convert a binary buffer (&Vec<u8>) into a struct
pub trait Deserialize {
    fn deserialize(buffer: &[u8]) -> Result<Self>
    where
        Self: Sized;
}

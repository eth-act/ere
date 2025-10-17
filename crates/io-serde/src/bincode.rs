use crate::IoSerde;
use alloc::vec::Vec;
use bincode::config::{Config, Configuration, Fixint, LittleEndian, NoLimit, Varint};
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use serde::{Deserialize, Serialize};

pub use bincode::{
    config,
    error::{DecodeError, EncodeError},
};

#[derive(Debug)]
pub enum BincodeError {
    Encode(EncodeError),
    Decode(DecodeError),
}

impl Display for BincodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "{err:?}"),
            Self::Decode(err) => write!(f, "{err:?}"),
        }
    }
}

impl Error for BincodeError {}

/// IO de/serialization implementation with [`bincode`].
#[derive(Clone, Copy, Debug)]
pub struct Bincode<O>(pub O);

impl Bincode<Configuration<LittleEndian, Fixint, NoLimit>> {
    /// `Bincode` with legacy configuration, same as the default of `bincode@1`.
    pub fn legacy() -> Self {
        Self(bincode::config::legacy())
    }
}

impl Bincode<Configuration<LittleEndian, Varint, NoLimit>> {
    /// `Bincode` with standard configuration.
    pub fn standard() -> Self {
        Self(bincode::config::standard())
    }
}

impl<O: Config> IoSerde for Bincode<O> {
    type Error = BincodeError;

    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Self::Error> {
        bincode::serde::encode_to_vec(value, self.0).map_err(BincodeError::Encode)
    }

    fn deserialize<'a, T: Deserialize<'a>>(&self, bytes: &'a [u8]) -> Result<T, Self::Error> {
        let (value, _) = bincode::serde::borrow_decode_from_slice(bytes, self.0)
            .map_err(BincodeError::Decode)?;
        Ok(value)
    }
}

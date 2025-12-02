use crate::serde::Serde;
use alloc::vec::Vec;
use bincode::config::{Config, Configuration, Fixint, LittleEndian, NoLimit, Varint};
use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
};
use serde::{Serialize, de::DeserializeOwned};

pub use bincode::{
    config,
    error::{DecodeError, EncodeError},
};

pub type BincodeLegacyConfig = Configuration<LittleEndian, Fixint, NoLimit>;
pub type BincodeLegacy = Bincode<BincodeLegacyConfig>;
pub type BincodeStandardConfig = Configuration<LittleEndian, Varint, NoLimit>;
pub type BincodeStandard = Bincode<BincodeStandardConfig>;

#[derive(Debug)]
pub enum BincodeError {
    Serialize(EncodeError),
    Deserialize(DecodeError),
}

impl Display for BincodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(err) => write!(f, "Bincode serialize error: {err:?}"),
            Self::Deserialize(err) => write!(f, "Bincode deserialize error: {err:?}"),
        }
    }
}

impl Error for BincodeError {}

/// IO de/serialization implementation with [`bincode`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Bincode<O>(pub O);

impl Bincode<BincodeLegacyConfig> {
    /// `Bincode` with legacy configuration, same as the default of `bincode@1`.
    pub fn legacy() -> Self {
        Self(bincode::config::legacy())
    }
}

impl Bincode<BincodeStandardConfig> {
    /// `Bincode` with standard configuration.
    pub fn standard() -> Self {
        Self(bincode::config::standard())
    }
}

impl<O: Default + Send + Sync + Config> Serde for Bincode<O> {
    type Error = BincodeError;

    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Self::Error> {
        bincode::serde::encode_to_vec(value, self.0).map_err(BincodeError::Serialize)
    }

    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, Self::Error> {
        let (value, _) =
            bincode::serde::decode_from_slice(bytes, self.0).map_err(BincodeError::Deserialize)?;
        Ok(value)
    }
}

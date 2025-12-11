use crate::serde::Serde;
use alloc::vec::Vec;
use ciborium_io::EndOfFile;
use core::{
    convert::Infallible,
    error::Error,
    fmt::{self, Display, Formatter},
};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Debug)]
pub enum CborError {
    Serialize(ciborium::ser::Error<Infallible>),
    Deserialize(ciborium::de::Error<EndOfFile>),
}

impl Display for CborError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(err) => write!(f, "CBOR serialize error: {err:?}"),
            Self::Deserialize(err) => write!(f, "CBOR deserialize error: {err:?}"),
        }
    }
}

impl Error for CborError {}

/// IO de/serialization implementation with [`ciborium`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Cbor;

impl Serde for Cbor {
    type Error = CborError;

    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Self::Error> {
        let mut buf = Vec::new();
        ciborium::into_writer(value, &mut buf).map_err(CborError::Serialize)?;
        Ok(buf)
    }

    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, Self::Error> {
        ciborium::from_reader(bytes).map_err(CborError::Deserialize)
    }
}

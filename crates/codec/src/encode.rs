use alloc::vec::Vec;
use core::{convert::Infallible, error::Error};

/// Serializes a value into the canonical byte representation for transport.
pub trait Encode {
    type Error: 'static + Send + Sync + Error;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error>;
}

impl Encode for () {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(Vec::new())
    }
}

impl Encode for Vec<u8> {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.clone())
    }
}

impl<const N: usize> Encode for [u8; N] {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.to_vec())
    }
}

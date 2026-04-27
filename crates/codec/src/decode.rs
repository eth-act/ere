use alloc::vec::Vec;
use core::{array::TryFromSliceError, convert::Infallible, error::Error};

/// Deserializes a value from its canonical byte representation.
pub trait Decode: Sized {
    type Error: 'static + Send + Sync + Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error>;
}

impl Decode for () {
    type Error = Infallible;

    fn decode_from_slice(_: &[u8]) -> Result<Self, Self::Error> {
        Ok(())
    }
}

impl Decode for Vec<u8> {
    type Error = Infallible;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        Ok(slice.to_vec())
    }
}

impl<const N: usize> Decode for [u8; N] {
    type Error = TryFromSliceError;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        slice.try_into()
    }
}

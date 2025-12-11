use crate::Io;
use alloc::vec::Vec;
use core::{error::Error, fmt::Debug, marker::PhantomData};
use serde::{Serialize, de::DeserializeOwned};

#[cfg(feature = "bincode")]
pub mod bincode;

#[cfg(feature = "cbor")]
pub mod cbor;

pub trait Serde: Clone + Default + Send + Sync {
    type Error: 'static + Error + Send + Sync;

    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Self::Error>;

    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, Self::Error>;
}

pub struct IoSerde<I, O, T>(PhantomData<(I, O, T)>);

impl<I, O, T> Io for IoSerde<I, O, T>
where
    I: Clone + Debug + Send + Sync + Serialize + DeserializeOwned,
    O: Clone + Debug + Send + Sync + PartialEq + Serialize + DeserializeOwned,
    T: Serde,
{
    type Input = I;
    type Output = O;
    type Error = T::Error;

    fn serialize_input(input: &Self::Input) -> Result<Vec<u8>, Self::Error> {
        T::default().serialize(input)
    }

    fn deserialize_input(bytes: &[u8]) -> Result<Self::Input, Self::Error> {
        T::default().deserialize(bytes)
    }

    fn serialize_output(output: &Self::Output) -> Result<Vec<u8>, Self::Error> {
        T::default().serialize(output)
    }

    fn deserialize_output(bytes: &[u8]) -> Result<Self::Output, Self::Error> {
        T::default().deserialize(bytes)
    }
}

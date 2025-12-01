use crate::Io;
use alloc::vec::Vec;
use core::{fmt::Debug, marker::PhantomData};
use rkyv::{
    Archive, Deserialize, Serialize,
    api::high::{HighSerializer, HighValidator},
    bytecheck::CheckBytes,
    de::Pool,
    rancor::{Error, Strategy},
    ser::allocator::ArenaHandle,
    util::AlignedVec,
};

pub use rkyv;

pub struct IoRkyv<I, O>(PhantomData<(I, O)>);

impl<I, O> Io for IoRkyv<I, O>
where
    I: Clone
        + Debug
        + Send
        + Sync
        + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, Error>>
        + Archive,
    I::Archived:
        for<'a> CheckBytes<HighValidator<'a, Error>> + Deserialize<I, Strategy<Pool, Error>>,
    O: Clone
        + Debug
        + Send
        + Sync
        + PartialEq
        + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, Error>>
        + Archive,
    O::Archived:
        for<'a> CheckBytes<HighValidator<'a, Error>> + Deserialize<O, Strategy<Pool, Error>>,
{
    type Input = I;
    type Output = O;
    type Error = Error;

    fn serialize_input(input: &Self::Input) -> Result<Vec<u8>, Self::Error> {
        rkyv::to_bytes(input).map(|vec| vec.to_vec())
    }

    fn deserialize_input(bytes: &[u8]) -> Result<Self::Input, Self::Error> {
        rkyv::from_bytes(bytes)
    }

    fn serialize_output(output: &Self::Output) -> Result<Vec<u8>, Self::Error> {
        rkyv::to_bytes(output).map(|vec| vec.to_vec())
    }

    fn deserialize_output(bytes: &[u8]) -> Result<Self::Output, Self::Error> {
        rkyv::from_bytes(bytes)
    }
}

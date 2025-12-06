//! This crate provides IO de/serialization implementation to be shared between
//! host and guest, if the guest is also written in Rust.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::{error::Error, fmt::Debug};

#[cfg(feature = "serde")]
pub mod serde;

#[cfg(feature = "rkyv")]
pub mod rkyv;

/// IO de/serialization to be shared between host and guest.
pub trait Io {
    type Input: Clone + Debug + Send + Sync;
    type Output: Clone + Debug + Send + Sync + PartialEq;
    type Error: 'static + Error + Send + Sync;

    fn serialize_input(input: &Self::Input) -> Result<Vec<u8>, Self::Error>;

    fn deserialize_input(bytes: &[u8]) -> Result<Self::Input, Self::Error>;

    fn serialize_output(output: &Self::Output) -> Result<Vec<u8>, Self::Error>;

    fn deserialize_output(bytes: &[u8]) -> Result<Self::Output, Self::Error>;
}

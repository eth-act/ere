//! This crate provides IO de/serialization implementation to be shared between
//! host and guest, if the guest is also written in Rust.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::error::Error;
use serde::{Serialize, de::DeserializeOwned};

#[cfg(feature = "bincode")]
pub mod bincode;

#[cfg(feature = "cbor")]
pub mod cbor;

/// IO de/serialization to be shared between host and guest.
pub trait IoSerde {
    type Error: Error;

    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Self::Error>;

    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, Self::Error>;
}

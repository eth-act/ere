#![cfg_attr(not(feature = "host"), no_std)]

extern crate alloc;

pub mod codec;
pub mod program;

#[cfg(feature = "host")]
pub mod host;

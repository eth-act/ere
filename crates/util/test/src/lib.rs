#![cfg_attr(not(feature = "host"), no_std)]

extern crate alloc;

pub mod program;

#[cfg(feature = "host")]
pub mod host;

pub use ere_io as io;

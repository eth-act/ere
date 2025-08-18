#![cfg_attr(not(feature = "host"), no_std)]

extern crate alloc;

pub mod guest;

#[cfg(feature = "host")]
pub mod host;

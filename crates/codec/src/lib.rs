#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

mod decode;
mod encode;
mod macros;

pub use crate::{decode::Decode, encode::Encode};

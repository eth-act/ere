#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

mod decode;
mod encode;
mod macros;

/// Upper bound on the bytes a bincode decode may claim from one input.
///
/// A length prefix in the input decides how much a container allocates, before any of that
/// length has been read. Without a bound, an input that declares a length it does not carry is
/// an allocation of that size, which fails the allocator and aborts the process rather than
/// returning an error. The bound turns that into `DecodeError::LimitExceeded`.
///
/// The largest artifact these implementations decode today is an SP1 compressed proof, a little
/// over one mebibyte, so this leaves roughly two orders of magnitude of headroom.
pub const MAX_DECODE_BYTES: usize = 64 * 1024 * 1024;

pub use crate::{decode::Decode, encode::Encode};

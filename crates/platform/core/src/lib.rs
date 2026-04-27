#![no_std]

mod length_prefixed_stdin;
mod platform;

pub use crate::{length_prefixed_stdin::LengthPrefixedStdin, platform::Platform};

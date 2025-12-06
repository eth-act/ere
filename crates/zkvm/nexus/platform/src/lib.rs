#![no_std]

extern crate alloc;

use core::ops::Deref;
use ere_platform_trait::LengthPrefixedStdin;

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use nexus_rt;

/// Nexus [`Platform`] implementation.
pub struct NexusPlatform;

impl Platform for NexusPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(nexus_rt::read_private_input().unwrap())
    }

    fn write_whole_output(output: &[u8]) {
        nexus_rt::write_public_output(output).unwrap()
    }

    fn print(message: &str) {
        nexus_rt::print!("{message}")
    }
}

#![no_std]

extern crate alloc;

use core::ops::Deref;
use ere_platform_trait::LengthPrefixedStdin;

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use zkm_zkvm;

/// Ziren [`Platform`] implementation.
pub struct ZirenPlatform;

impl Platform for ZirenPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(zkm_zkvm::io::read_vec())
    }

    fn write_whole_output(output: &[u8]) {
        zkm_zkvm::io::commit_slice(output);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        zkm_zkvm::syscalls::sys_write(1, bytes.as_ptr(), bytes.len());
    }
}

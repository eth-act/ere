#![no_std]

extern crate alloc;

use alloc::vec;
use core::{ops::Deref, slice};
use risc0_zkvm::guest::env::Write;

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use risc0_zkvm;

/// Risc0 [`Platform`] implementation.
pub struct Risc0Platform;

impl Platform for Risc0Platform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        let mut len = 0u32;
        risc0_zkvm::guest::env::read_slice(slice::from_mut(&mut len));
        let mut input = vec![0u8; len as usize];
        risc0_zkvm::guest::env::read_slice(&mut input);
        input
    }

    fn write_whole_output(output: &[u8]) {
        risc0_zkvm::guest::env::commit_slice(output);
    }

    fn print(message: &str) {
        risc0_zkvm::guest::env::stdout().write_slice(message.as_bytes());
    }

    fn cycle_count() -> u64 {
        risc0_zkvm::guest::env::cycle_count()
    }
}

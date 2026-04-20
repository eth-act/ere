#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

use alloc::vec;
use core::ops::Deref;

pub use ere_platform_core::Platform;
pub use risc0_zkvm;
use risc0_zkvm::guest::env::Write;
use risc0_zkvm_platform as _;

/// Risc0 [`Platform`] implementation.
pub struct Risc0Platform;

impl Platform for Risc0Platform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        let len = {
            let mut bytes = [0; 4];
            risc0_zkvm::guest::env::read_slice(&mut bytes);
            u32::from_le_bytes(bytes) as usize
        };
        let mut input = vec![0u8; len];
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

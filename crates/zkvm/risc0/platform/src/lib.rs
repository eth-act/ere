#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::OutputHasher;
use risc0_zkvm::guest::env::Write;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use risc0_zkvm;

/// Risc0 [`Platform`] implementation.
pub struct Risc0Platform<H = IdentityOutput>(PhantomData<H>);

impl<H: OutputHasher> Platform for Risc0Platform<H> {
    fn read_whole_input() -> Vec<u8> {
        let len = {
            let mut bytes = [0; 4];
            risc0_zkvm::guest::env::read_slice(&mut bytes);
            u32::from_le_bytes(bytes)
        };
        let mut input = vec![0u8; len as usize];
        risc0_zkvm::guest::env::read_slice(&mut input);
        input
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        risc0_zkvm::guest::env::commit_slice(&hash);
    }

    fn print(message: &str) {
        risc0_zkvm::guest::env::stdout().write_slice(message.as_bytes());
    }

    fn cycle_count() -> u64 {
        risc0_zkvm::guest::env::cycle_count()
    }
}

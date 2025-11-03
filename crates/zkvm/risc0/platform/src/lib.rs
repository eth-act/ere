#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};

pub use ere_platform_trait::Platform;
pub use risc0_zkvm;

pub struct Risc0Platform;

impl Platform for Risc0Platform {
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
        risc0_zkvm::guest::env::commit_slice(output);
    }
}

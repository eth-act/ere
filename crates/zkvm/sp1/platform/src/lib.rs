#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub use ere_platform_trait::Platform;
pub use sp1_zkvm;

pub struct SP1Platform;

impl Platform for SP1Platform {
    fn read_whole_input() -> Vec<u8> {
        sp1_zkvm::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        sp1_zkvm::io::commit_slice(output);
    }
}

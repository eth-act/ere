#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub use ere_platform_trait::Platform;
pub use zkm_zkvm;

pub struct ZirenPlatform;

impl Platform for ZirenPlatform {
    fn read_whole_input() -> Vec<u8> {
        zkm_zkvm::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        zkm_zkvm::io::commit_slice(output);
    }
}

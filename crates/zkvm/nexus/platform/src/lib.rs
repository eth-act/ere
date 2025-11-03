#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub use ere_platform_trait::Platform;
pub use nexus_rt;

pub struct NexusPlatform;

impl Platform for NexusPlatform {
    fn read_whole_input() -> Vec<u8> {
        nexus_rt::read_private_input().unwrap()
    }

    fn write_whole_output(output: &[u8]) {
        nexus_rt::write_public_output(&output).unwrap()
    }
}

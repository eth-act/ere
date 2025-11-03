#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub use ere_platform_trait::Platform;
pub use pico_sdk;

pub struct PicoPlatform;

impl Platform for PicoPlatform {
    fn read_whole_input() -> Vec<u8> {
        pico_sdk::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        pico_sdk::io::commit_bytes(output);
    }
}

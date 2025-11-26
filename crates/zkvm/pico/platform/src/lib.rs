#![no_std]

extern crate alloc;

use alloc::{format, vec::Vec};
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::OutputHasher;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use pico_sdk;

pub struct PicoPlatform<H = IdentityOutput>(PhantomData<H>);

impl<H: OutputHasher> Platform for PicoPlatform<H> {
    fn read_whole_input() -> Vec<u8> {
        pico_sdk::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        pico_sdk::io::commit_bytes(&hash);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        pico_sdk::riscv_ecalls::sys_write(1, bytes.as_ptr(), bytes.len());
    }

    fn cycle_scope_start(name: &str) {
        Self::print(&format!("cycle-tracker-start: {name}"))
    }

    fn cycle_scope_end(name: &str) {
        Self::print(&format!("cycle-tracker-end: {name}"))
    }
}

#![no_std]

extern crate alloc;

use alloc::{format, vec::Vec};
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::OutputHasher;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use sp1_zkvm;

pub struct SP1Platform<H = IdentityOutput>(PhantomData<H>);

impl<H: OutputHasher> Platform for SP1Platform<H> {
    fn read_whole_input() -> Vec<u8> {
        sp1_zkvm::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        sp1_zkvm::io::commit_slice(&hash);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        sp1_zkvm::syscalls::sys_write(1, bytes.as_ptr(), bytes.len());
    }

    fn cycle_scope_start(name: &str) {
        Self::print(&format!("cycle-tracker-report-start: {name}"))
    }

    fn cycle_scope_end(name: &str) {
        Self::print(&format!("cycle-tracker-report-end: {name}"))
    }
}

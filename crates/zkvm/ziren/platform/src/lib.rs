#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::OutputHasher;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use zkm_zkvm;

pub struct ZirenPlatform<H = IdentityOutput>(PhantomData<H>);

impl<H: OutputHasher> Platform for ZirenPlatform<H> {
    fn read_whole_input() -> Vec<u8> {
        zkm_zkvm::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        zkm_zkvm::io::commit_slice(&hash);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        zkm_zkvm::syscalls::sys_write(1, bytes.as_ptr(), bytes.len());
    }
}

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::OutputHasher;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use nexus_rt;

/// Nexus [`Platform`] implementation.
pub struct NexusPlatform<H = IdentityOutput>(PhantomData<H>);

impl<H: OutputHasher> Platform for NexusPlatform<H> {
    fn read_whole_input() -> Vec<u8> {
        nexus_rt::read_private_input().unwrap()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        nexus_rt::write_public_output(&*hash).unwrap()
    }

    fn print(message: &str) {
        nexus_rt::print!("{message}")
    }
}

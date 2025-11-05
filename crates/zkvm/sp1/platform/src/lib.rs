#![no_std]

extern crate alloc;

use alloc::vec::Vec;
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
        sp1_zkvm::io::commit_slice(&*hash);
    }
}

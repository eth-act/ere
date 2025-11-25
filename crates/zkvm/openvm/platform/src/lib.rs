#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::{marker::PhantomData, ops::Deref};
use ere_platform_trait::output_hasher::FixedOutputHasher;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum::U32},
};
pub use openvm;

pub struct OpenVMPlatform<H>(PhantomData<H>);

impl<H: FixedOutputHasher<OutputSize = U32>> Platform for OpenVMPlatform<H> {
    fn read_whole_input() -> Vec<u8> {
        openvm::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output).deref().try_into().unwrap();
        openvm::io::reveal_bytes32(hash);
    }

    fn print(message: &str) {
        openvm::io::print(message)
    }
}

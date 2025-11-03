#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::{OutputHasher, digest::typenum::U32};

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput},
};
pub use openvm;

pub struct OpenVMPlatform<D>(PhantomData<D>);

impl<D: OutputHasher<OutputSize = U32>> Platform for OpenVMPlatform<D> {
    fn read_whole_input() -> Vec<u8> {
        openvm::io::read_vec()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = D::output_hash(output);
        openvm::io::reveal_bytes32(hash.into());
    }
}

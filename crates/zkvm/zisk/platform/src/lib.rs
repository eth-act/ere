#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::{OutputHasher, digest::typenum::U32};

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput},
};
pub use ziskos;

pub struct ZiskPlatform<D>(PhantomData<D>);

impl<D: OutputHasher<OutputSize = U32>> Platform for ZiskPlatform<D> {
    fn read_whole_input() -> Vec<u8> {
        ziskos::read_input()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = D::output_hash(output);
        hash.chunks_exact(4).enumerate().for_each(|(idx, bytes)| {
            ziskos::set_output(idx, u32::from_le_bytes(bytes.try_into().unwrap()))
        });
    }
}

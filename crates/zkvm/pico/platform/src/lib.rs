#![no_std]

extern crate alloc;

use alloc::vec::Vec;
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
        pico_sdk::io::commit_bytes(&*hash);
    }
}

#![no_std]

extern crate alloc;

use core::{marker::PhantomData, ops::Deref};
use ere_platform_trait::{LengthPrefixedStdin, output_hasher::OutputHasher};

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use nexus_rt;

/// Nexus [`Platform`] implementation.
pub struct NexusPlatform<H = IdentityOutput>(PhantomData<H>);

impl<H: OutputHasher> Platform for NexusPlatform<H> {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(nexus_rt::read_private_input().unwrap())
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        nexus_rt::write_public_output(&*hash).unwrap()
    }

    fn print(message: &str) {
        nexus_rt::print!("{message}")
    }
}

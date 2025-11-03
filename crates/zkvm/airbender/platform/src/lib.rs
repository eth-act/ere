#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::{array, iter::repeat_with, marker::PhantomData};
use ere_platform_trait::output_hasher::{OutputHasher, digest::typenum::U32};

pub use airbender_riscv_common as riscv_common;
pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput},
};

pub struct AirbenderPlatform<D>(PhantomData<D>);

impl<D: OutputHasher<OutputSize = U32>> Platform for AirbenderPlatform<D> {
    fn read_whole_input() -> Vec<u8> {
        let len = riscv_common::csr_read_word() as usize;
        repeat_with(riscv_common::csr_read_word)
            .take(len.div_ceil(4))
            .flat_map(u32::to_le_bytes)
            .take(len)
            .collect()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = D::output_hash(output);
        let words = array::from_fn(|i| u32::from_le_bytes(array::from_fn(|j| hash[4 * i + j])));
        riscv_common::zksync_os_finish_success(&words);
    }
}

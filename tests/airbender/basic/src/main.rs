#![no_std]
#![no_main]
#![no_builtins]
#![allow(incomplete_features)]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]

extern crate alloc;

use alloc::vec::Vec;
use core::{array, iter::repeat_with};
use ere_test_utils::{
    guest::{Digest, Platform, Sha256},
    program::{basic::BasicProgram, Program},
};
use riscv_common::{csr_read_word, zksync_os_finish_success};

mod airbender_rt;

struct AirbenderPlatform;

impl Platform for AirbenderPlatform {
    fn read_input() -> Vec<u8> {
        let len = csr_read_word() as usize;
        repeat_with(csr_read_word)
            .take(len.div_ceil(4))
            .flat_map(|word| word.to_le_bytes())
            .take(len)
            .collect()
    }

    fn write_output(output: &[u8]) {
        let digest = Sha256::digest(output);
        let words = array::from_fn(|i| u32::from_le_bytes(array::from_fn(|j| digest[4 * i + j])));
        zksync_os_finish_success(&words);
    }
}

#[inline(never)]
fn main() {
    BasicProgram::run::<AirbenderPlatform>();
}

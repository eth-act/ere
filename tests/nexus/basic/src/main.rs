#![cfg_attr(target_arch = "riscv32", no_std, no_main)]

extern crate alloc;

use alloc::vec::Vec;
use ere_test_utils::{
    guest::Platform,
    program::{basic::BasicProgram, Program},
};
use nexus_rt::{read_private_input, write_public_output};

struct NexusPlatform;

impl Platform for NexusPlatform {
    fn read_input() -> Vec<u8> {
        read_private_input().unwrap()
    }

    fn write_output(output: &[u8]) {
        write_public_output(&output).unwrap()
    }
}

#[nexus_rt::main]
fn main() {
    BasicProgram::run::<NexusPlatform>();
}

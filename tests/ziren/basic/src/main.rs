#![no_main]

use ere_test_utils::{
    guest::Platform,
    program::{basic::BasicProgram, Program},
};

zkm_zkvm::entrypoint!(main);

struct ZirenPlatform;

impl Platform for ZirenPlatform {
    fn read_input() -> Vec<u8> {
        zkm_zkvm::io::read_vec()
    }

    fn write_output(output: &[u8]) {
        zkm_zkvm::io::commit_slice(output);
    }
}

pub fn main() {
    BasicProgram::run::<ZirenPlatform>();
}

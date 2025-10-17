#![no_main]

use ere_test_utils::{
    guest::Platform,
    program::{basic::BasicProgram, Program},
};

sp1_zkvm::entrypoint!(main);

struct SP1Platform;

impl Platform for SP1Platform {
    fn read_input() -> Vec<u8> {
        sp1_zkvm::io::read_vec()
    }

    fn write_output(output: &[u8]) {
        sp1_zkvm::io::commit_slice(output);
    }
}

pub fn main() {
    BasicProgram::run::<SP1Platform>();
}

use ere_test_utils::{
    guest::Platform,
    program::{basic::BasicProgram, Program},
};
use risc0_zkvm::guest::env;
use std::io::Read;

struct Risc0Platform;

impl Platform for Risc0Platform {
    fn read_input() -> Vec<u8> {
        let mut input = Vec::new();
        env::stdin().read_to_end(&mut input).unwrap();
        input
    }

    fn write_output(output: &[u8]) {
        env::commit_slice(output);
    }
}

fn main() {
    BasicProgram::run::<Risc0Platform>();
}

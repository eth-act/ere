#![no_main]

use ere_test_utils::{
    guest::Platform,
    program::{basic::BasicProgram, Program},
};
use pico_sdk::io::{commit_bytes, read_vec};

pico_sdk::entrypoint!(main);

struct PicoPlatform;

impl Platform for PicoPlatform {
    fn read_input() -> Vec<u8> {
        read_vec()
    }

    fn write_output(output: &[u8]) {
        commit_bytes(output);
    }
}

pub fn main() {
    BasicProgram::run::<PicoPlatform>();
}

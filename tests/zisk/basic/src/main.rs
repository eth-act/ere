#![no_main]

use ere_test_utils::{
    guest::{Digest, Platform, Sha256},
    program::{basic::BasicProgram, Program},
};

ziskos::entrypoint!(main);

struct ZiskPlatform;

impl Platform for ZiskPlatform {
    fn read_input() -> Vec<u8> {
        ziskos::read_input()
    }

    fn write_output(output: &[u8]) {
        let digest = Sha256::digest(output);
        digest.chunks_exact(4).enumerate().for_each(|(idx, bytes)| {
            ziskos::set_output(idx, u32::from_le_bytes(bytes.try_into().unwrap()))
        });
    }
}

fn main() {
    BasicProgram::run::<ZiskPlatform>();
}

use ere_test_utils::{
    guest::{Digest, Platform, Sha256},
    program::{basic::BasicProgram, Program},
};
use openvm::io::{read_vec, reveal_bytes32};

struct OpenVMPlatform;

impl Platform for OpenVMPlatform {
    fn read_input() -> Vec<u8> {
        read_vec()
    }

    fn write_output(output: &[u8]) {
        let digest = Sha256::digest(output);
        reveal_bytes32(digest.into());
    }
}

fn main() {
    BasicProgram::run::<OpenVMPlatform>();
}

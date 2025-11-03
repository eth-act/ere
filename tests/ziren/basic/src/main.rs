#![no_main]

use ere_platform_ziren::{zkm_zkvm, ZirenPlatform};
use ere_test_utils::program::{basic::BasicProgram, Program};

zkm_zkvm::entrypoint!(main);

pub fn main() {
    BasicProgram::run::<ZirenPlatform>();
}

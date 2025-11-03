#![no_main]

use ere_platform_sp1::{sp1_zkvm, SP1Platform};
use ere_test_utils::program::{basic::BasicProgram, Program};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    BasicProgram::run::<SP1Platform>();
}

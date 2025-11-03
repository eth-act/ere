use ere_platform_risc0::Risc0Platform;
use ere_test_utils::program::{basic::BasicProgram, Program};

fn main() {
    BasicProgram::run::<Risc0Platform>();
}

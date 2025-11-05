#![cfg_attr(target_arch = "riscv32", no_std, no_main)]

use ere_platform_nexus::{nexus_rt, NexusPlatform};
use ere_test_utils::program::{basic::BasicProgram, Program};

#[nexus_rt::main]
fn main() {
    BasicProgram::run::<NexusPlatform>();
}

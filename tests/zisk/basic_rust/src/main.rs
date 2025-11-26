#![no_main]

use ere_platform_zisk::{ziskos, ZiskPlatform};
use ere_test_utils::program::{basic::BasicProgram, Program};

ziskos::entrypoint!(main);

fn main() {
    BasicProgram::run::<ZiskPlatform>();
}

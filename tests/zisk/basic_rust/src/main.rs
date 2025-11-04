#![no_main]

use ere_platform_zisk::{ziskos, ZiskPlatform};
use ere_test_utils::{
    guest::Sha256,
    program::{basic::BasicProgram, Program},
};

ziskos::entrypoint!(main);

fn main() {
    BasicProgram::run::<ZiskPlatform<Sha256>>();
}

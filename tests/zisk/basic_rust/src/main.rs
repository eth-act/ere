#![no_main]

use ere_platform_zisk::{ziskos, ZiskPlatform};
use ere_test_utils::{
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

ziskos::entrypoint!(main);

fn main() {
    BasicProgram::<BincodeLegacy>::run::<ZiskPlatform>();
}

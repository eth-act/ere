#![no_main]

use ere_platform_pico::{pico_sdk, PicoPlatform};
use ere_test_utils::{
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

pico_sdk::entrypoint!(main);

pub fn main() {
    BasicProgram::<BincodeLegacy>::run::<PicoPlatform>();
}

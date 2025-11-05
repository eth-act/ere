#![no_main]

use ere_platform_pico::{pico_sdk, PicoPlatform};
use ere_test_utils::program::{basic::BasicProgram, Program};

pico_sdk::entrypoint!(main);

pub fn main() {
    BasicProgram::run::<PicoPlatform>();
}

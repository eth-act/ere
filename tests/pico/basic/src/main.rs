#![no_main]

use ere_platform_pico::{pico_sdk, PicoPlatform};
use ere_util_test::{
    codec::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

pico_sdk::entrypoint!(main);

fn main() {
    BasicProgram::<BincodeLegacy>::run::<PicoPlatform>();
}

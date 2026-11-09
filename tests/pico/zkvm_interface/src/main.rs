#![no_main]

use ere_platform_pico::{pico_sdk, PicoPlatform};
use ere_util_test::program::{zkvm_interface::ZkvmInterfaceProgram, Program};

pico_sdk::entrypoint!(main);

fn main() {
    ZkvmInterfaceProgram::run::<PicoPlatform>();
}

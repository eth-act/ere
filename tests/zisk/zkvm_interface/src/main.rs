#![no_main]

use ere_platform_zisk::{ZiskPlatform, ziskos};
use ere_util_test::program::{Program, zkvm_interface::ZkvmInterfaceProgram};

ziskos::entrypoint!(main);

fn main() {
    ZkvmInterfaceProgram::run::<ZiskPlatform>();
}

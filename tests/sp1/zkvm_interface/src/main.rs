#![no_main]

use ere_platform_sp1::{sp1_zkvm, SP1Platform};
use ere_util_test::program::{zkvm_interface::ZkvmInterfaceProgram, Program};

sp1_zkvm::entrypoint!(main);

fn main() {
    ZkvmInterfaceProgram::run::<SP1Platform>();
}

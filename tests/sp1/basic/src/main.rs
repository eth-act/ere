#![no_main]

use ere_platform_sp1::{sp1_zkvm, SP1Platform};
use ere_util_test::{
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    BasicProgram::<BincodeLegacy>::run::<SP1Platform>();
}

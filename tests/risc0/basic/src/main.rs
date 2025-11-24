use ere_platform_risc0::Risc0Platform;
use ere_test_utils::{
    io_serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::<BincodeLegacy>::run::<Risc0Platform>();
}

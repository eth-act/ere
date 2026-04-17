use ere_platform_risc0::Risc0Platform;
use ere_util_test::{
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::<BincodeLegacy>::run::<Risc0Platform>();
}

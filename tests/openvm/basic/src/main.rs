use ere_platform_openvm::OpenVMPlatform;
use ere_test_utils::{
    guest::Sha256,
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::<BincodeLegacy>::run::<OpenVMPlatform<Sha256>>();
}

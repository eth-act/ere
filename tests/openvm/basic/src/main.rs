use ere_platform_openvm::OpenVMPlatform;
use ere_test_utils::{
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::<BincodeLegacy>::run_output_sha256::<OpenVMPlatform>();
}

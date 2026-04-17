use ere_platform_openvm::OpenVMPlatform;
use ere_util_test::{
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::<BincodeLegacy>::run_output_sha256::<OpenVMPlatform>();
}

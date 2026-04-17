use ere_platform_openvm::OpenVMPlatform;
use ere_util_test::{
    codec::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::<BincodeLegacy>::run_output_sha256::<OpenVMPlatform>();
}

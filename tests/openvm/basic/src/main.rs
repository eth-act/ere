use ere_platform_openvm::OpenVMPlatform;
use ere_test_utils::{
    guest::Sha256,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::run::<OpenVMPlatform<Sha256>>();
}

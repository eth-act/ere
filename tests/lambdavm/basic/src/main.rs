use ere_platform_lambdavm::LambdaVMPlatform;
use ere_util_test::{
    codec::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

fn main() {
    BasicProgram::<BincodeLegacy>::run::<LambdaVMPlatform>();
}

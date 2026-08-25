use ere_platform_openvm::OpenVMPlatform;
use ere_util_test::program::{Program, zkvm_interface::ZkvmInterfaceProgram};

fn main() {
    ZkvmInterfaceProgram::run::<OpenVMPlatform>();
}

use crate::{
    compiler::SP1Program,
    error::{CompileError, SP1Error},
};
use compile_utils::CargoBuildCmd;
use std::{env, path::Path};
use zkvm_interface::Compiler;

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic", // Only for rustc > 1.81
    "-C",
    // Start of the code section
    "link-arg=-Ttext=0x00201000",
    "-C",
    // The lowest memory location that will be used when your program is loaded
    "link-arg=--image-base=0x00200800",
    "-C",
    "panic=abort",
    "--cfg",
    "getrandom_backend=\"custom\"",
    "-C",
    "llvm-args=-misched-prera-direction=bottomup",
    "-C",
    "llvm-args=-misched-postra-direction=bottomup",
];

const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

/// Compiler for Rust guest program to RV32IMA architecture.
pub struct RustRv32ima;

impl Compiler for RustRv32ima {
    type Error = SP1Error;

    type Program = SP1Program;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)
            .map_err(CompileError::CompileUtilError)?;
        Ok(elf)
    }
}

#[cfg(test)]
mod tests {
    use crate::{EreSP1, compiler::RustRv32ima};
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("sp1", "stock_nightly_no_std");
        let elf = RustRv32ima.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("sp1", "stock_nightly_no_std");
        let program = RustRv32ima.compile(&guest_directory).unwrap();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        zkvm.execute(&Input::new()).unwrap();
    }
}

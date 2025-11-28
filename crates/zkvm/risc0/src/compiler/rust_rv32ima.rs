use crate::{compiler::Error, program::Risc0Program};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::compiler::Compiler;
use risc0_binfmt::ProgramBinary;
use std::{env, path::Path};
use tracing::info;

// TODO: Make this with `zkos` package building to avoid binary file storing in repo.
// File taken from https://github.com/risc0/risc0/blob/v3.0.4/risc0/zkos/v1compat/elfs/v1compat.elf
const V1COMPAT_ELF: &[u8] = include_bytes!("rust_rv32ima/v1compat.elf");
const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/risc0/risc0/blob/v3.0.4/risc0/build/src/lib.rs#L455
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic", // Only for rustc > 1.81
    "-C",
    // Start of the code section
    "link-arg=-Ttext=0x00200800",
    "-C",
    "link-arg=--fatal-warnings",
    "-C",
    "panic=abort",
    "--cfg",
    "getrandom_backend=\"custom\"",
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

/// Compiler for Rust guest program to RV32IMA architecture.
pub struct RustRv32ima;

impl Compiler for RustRv32ima {
    type Error = Error;

    type Program = Risc0Program;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)?;

        let program = ProgramBinary::new(elf.as_slice(), V1COMPAT_ELF);
        let image_id = program
            .compute_image_id()
            .map_err(Error::ImageIDCalculationFailure)?;

        info!("Risc0 program compiled OK - {} bytes", elf.len());
        info!("Image ID - {image_id}");

        Ok(Risc0Program {
            elf: program.encode(),
            image_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv32ima, zkvm::EreRisc0};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{
        compiler::Compiler,
        zkvm::{ProverResourceType, zkVM},
    };

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("risc0", "stock_nightly_no_std");
        let program = RustRv32ima.compile(&guest_directory).unwrap();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("risc0", "stock_nightly_no_std");
        let program = RustRv32ima.compile(&guest_directory).unwrap();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        zkvm.execute(&[]).unwrap();
    }
}

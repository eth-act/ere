use crate::{compiler::Error, program::JoltProgram};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::compiler::Compiler;
use std::{env, path::Path};

const TARGET_TRIPLE: &str = "riscv64imac-unknown-none-elf";
// According to https://github.com/a16z/jolt/blob/35d46f5/jolt-core/src/host/program.rs#L96
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic",
    "-C",
    "panic=abort",
    "--cfg",
    "getrandom_backend=\"custom\"",
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

const LINKER_SCRIPT: &str = include_str!("rust_rv64imac/link.x");

/// Compiler for Rust guest program to RV64IMAC architecture.
pub struct RustRv64imac;

impl Compiler for RustRv64imac {
    type Error = Error;

    type Program = JoltProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .linker_script(Some(LINKER_SCRIPT))
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)?;
        Ok(JoltProgram { elf })
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv64imac, zkvm::EreJolt};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{
        Input,
        compiler::Compiler,
        zkvm::{ProverResource, zkVM},
    };

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("jolt", "stock_nightly_no_std");
        let program = RustRv64imac.compile(&guest_directory).unwrap();
        assert!(!program.elf().is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("jolt", "stock_nightly_no_std");
        let program = RustRv64imac.compile(&guest_directory).unwrap();
        let zkvm = EreJolt::new(program, ProverResource::Cpu).unwrap();

        zkvm.execute(&Input::new()).unwrap();
    }
}

use crate::{compiler::NexusProgram, error::CompileError};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::Compiler;
use std::path::Path;

const TARGET_TRIPLE: &str = "riscv32i-unknown-none-elf";
// Linker script from nexus-sdk
// https://github.com/nexus-xyz/nexus-zkvm/blob/v0.3.4/sdk/src/compile/linker-scripts/default.x
const LINKER_SCRIPT: &str = include_str!("rust_rv32i/linker.x");
const RUSTFLAGS: &[&str] = &["-C", "relocation-model=pic", "-C", "panic=abort"];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

/// Compiler for Rust guest program to RV32I architecture.
pub struct RustRv32i;

impl Compiler for RustRv32i {
    type Error = CompileError;

    type Program = NexusProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let elf = CargoBuildCmd::new()
            .linker_script(Some(LINKER_SCRIPT))
            // The compiled ELF will be incompatible with Nexus VM if we don't pin this version
            // https://github.com/nexus-xyz/nexus-zkvm/blob/main/rust-toolchain.toml
            .toolchain("nightly-2025-04-06")
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)?;
        Ok(elf)
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv32i;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("nexus", "basic");
        let elf = RustRv32i.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

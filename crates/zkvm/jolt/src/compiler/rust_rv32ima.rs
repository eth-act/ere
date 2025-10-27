use crate::{compiler::JoltProgram, error::CompileError};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::Compiler;
use std::{env, path::Path};

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// According to https://github.com/a16z/jolt/blob/55b9830a3944dde55d33a55c42522b81dd49f87a/jolt-core/src/host/mod.rs#L95
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic",
    "-C",
    "panic=abort",
    "-C",
    "strip=symbols",
    "-C",
    "opt-level=z",
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    "--features",
    "guest",
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

const DEFAULT_MEMORY_SIZE: u64 = 10 * 1024 * 1024;
const DEFAULT_STACK_SIZE: u64 = 4096;
const LINKER_SCRIPT_TEMPLATE: &str = include_str!("rust_rv32ima/template.ld");

fn make_linker_script() -> String {
    LINKER_SCRIPT_TEMPLATE
        .replace("{MEMORY_SIZE}", &DEFAULT_MEMORY_SIZE.to_string())
        .replace("{STACK_SIZE}", &DEFAULT_STACK_SIZE.to_string())
}

/// Compiler for Rust guest program to RV32IMA architecture.
pub struct RustRv32ima;

impl Compiler for RustRv32ima {
    type Error = CompileError;

    type Program = JoltProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .linker_script(Some(make_linker_script()))
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)?;
        Ok(elf)
    }
}

#[cfg(test)]
mod tests {
    use crate::{EreJolt, compiler::RustRv32ima};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{Compiler, ProverResourceType, zkVM};

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("jolt", "stock_nightly_no_std");
        let elf = RustRv32ima.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("jolt", "stock_nightly_no_std");
        let program = RustRv32ima.compile(&guest_directory).unwrap();
        let zkvm = EreJolt::new(program, ProverResourceType::Cpu).unwrap();

        zkvm.execute(&[]).unwrap();
    }
}

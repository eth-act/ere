use crate::{compiler::Error, program::JoltProgram};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::compiler::Compiler;
use jolt_common::constants::{
    DEFAULT_MEMORY_SIZE, DEFAULT_STACK_SIZE, EMULATOR_MEMORY_CAPACITY, STACK_CANARY_SIZE,
};
use std::{env, path::Path};

const TARGET_TRIPLE: &str = "riscv64imac-unknown-none-elf";
// According to https://github.com/a16z/jolt/blob/v0.3.0-alpha/jolt-core/src/host/program.rs#L82
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic",
    "-C",
    "panic=abort",
    "-C",
    "strip=symbols",
    "-C",
    "opt-level=z",
    "--cfg",
    "getrandom_backend=\"custom\"",
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

const LINKER_SCRIPT_TEMPLATE: &str = include_str!("rust_rv64imac/template.ld");

fn make_linker_script() -> String {
    LINKER_SCRIPT_TEMPLATE
        .replace("{EMULATOR_MEMORY}", &EMULATOR_MEMORY_CAPACITY.to_string())
        .replace("{STACK_CANARY}", &STACK_CANARY_SIZE.to_string())
        .replace("{MEMORY_SIZE}", &DEFAULT_MEMORY_SIZE.to_string())
        .replace("{STACK_SIZE}", &DEFAULT_STACK_SIZE.to_string())
}

/// Compiler for Rust guest program to RV64IMAC architecture.
pub struct RustRv64imac;

impl Compiler for RustRv64imac {
    type Error = Error;

    type Program = JoltProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .linker_script(Some(make_linker_script()))
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
        zkvm::{ProverResourceType, zkVM},
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
        let zkvm = EreJolt::new(program, ProverResourceType::Cpu).unwrap();

        zkvm.execute(&Input::default()).unwrap();
    }
}

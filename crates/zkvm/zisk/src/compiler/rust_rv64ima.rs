use crate::{compiler::Error, program::ZiskProgram};
use ere_compile_utils::{CargoBuildCmd, RustTarget};
use ere_zkvm_interface::compiler::Compiler;
use std::{env, path::Path};

/// Target spec from the customized ZisK Rust toolchain.
///
/// To reproduce:
///
/// ```bash
/// rustc +zisk -Z unstable-options --print target-spec-json --target riscv64ima-zisk-zkvm-elf \
///     > ./crates/zkvm/zisk/src/compiler/rust_rv64ima/riscv64ima-unknown-none-elf.json
/// ```
const TARGET: RustTarget = RustTarget::SpecJson {
    name: "riscv64ima-unknown-none-elf",
    json: include_str!("./rust_rv64ima/riscv64ima-unknown-none-elf.json"),
};

const RUSTFLAGS: &[&str] = &["-C", "panic=abort", "--cfg", "getrandom_backend=\"custom\""];

const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
    // For using json target spec
    "-Zjson-target-spec",
];

/// Compiler for Rust guest program to RV64IMA architecture, using a stock
/// nightly Rust toolchain with ZisK's target specification.
pub struct RustRv64ima;

impl Compiler for RustRv64ima {
    type Error = Error;

    type Program = ZiskProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET)?;
        Ok(ZiskProgram { elf })
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv64ima, zkvm::EreZisk};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{
        Input,
        compiler::Compiler,
        zkvm::{ProverResource, zkVM},
    };

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("zisk", "stock_nightly_no_std");
        let program = RustRv64ima.compile(&guest_directory).unwrap();
        assert!(!program.elf().is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("zisk", "stock_nightly_no_std");
        let program = RustRv64ima.compile(&guest_directory).unwrap();
        let zkvm = EreZisk::new(program, ProverResource::Cpu).unwrap();

        zkvm.execute(&Input::new()).unwrap();
    }
}

use crate::{compiler::Error, program::SP1Program};
use ere_compile_utils::{CargoBuildCmd, RustTarget};
use ere_zkvm_interface::compiler::Compiler;
use std::{env, path::Path};

/// Target spec modified from `riscv64im-unknown-none-elf` with patch `atomic-cas = true`.
///
/// To reproduce:
///
/// ```bash
/// rustc +nightly -Z unstable-options --print target-spec-json --target riscv64im-unknown-none-elf \
///     | jq '.["atomic-cas"] = true' \
///     > ./crates/zkvm/sp1/src/compiler/rust_rv64ima/riscv64ima-unknown-none-elf.json
/// ```
const TARGET: RustTarget = RustTarget::SpecJson {
    name: "riscv64ima-unknown-none-elf",
    json: include_str!("./rust_rv64ima/riscv64ima-unknown-none-elf.json"),
};

/// According to https://github.com/succinctlabs/sp1/blob/v6.0.1/crates/build/src/command/utils.rs#L49.
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic", // Only for rustc > 1.81
    // The lowest memory location that will be used when your program is loaded
    "-C",
    "link-arg=--image-base=0x78000000",
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
    // For using json target spec
    "-Zjson-target-spec",
];

/// Compiler for Rust guest program to RV64IMA architecture.
pub struct RustRv64ima;

impl Compiler for RustRv64ima {
    type Error = Error;

    type Program = SP1Program;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET)?;
        Ok(SP1Program { elf })
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv64ima, zkvm::EreSP1};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{
        Input,
        compiler::Compiler,
        zkvm::{ProverResource, zkVM},
    };

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("sp1", "stock_nightly_no_std");
        let program = RustRv64ima.compile(&guest_directory).unwrap();
        assert!(!program.elf().is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("sp1", "stock_nightly_no_std");
        let program = RustRv64ima.compile(&guest_directory).unwrap();
        let zkvm = EreSP1::new(program, ProverResource::Cpu).unwrap();

        zkvm.execute(&Input::new()).unwrap();
    }
}

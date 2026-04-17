use crate::compiler::Error;
use ere_prover_core::compiler::{Compiler, Elf};
use ere_util_compile::{CargoBuildCmd, RustTarget};
use std::{env, path::Path};

/// Target spec modified from `riscv64im-unknown-none-elf` with patch `atomic-cas = true`.
///
/// To reproduce:
///
/// ```bash
/// rustc +nightly -Z unstable-options --print target-spec-json --target riscv64im-unknown-none-elf \
///     | jq '.["atomic-cas"] = true' \
///     > ./crates/prover/zisk/src/compiler/rust_rv64ima/riscv64ima-unknown-none-elf.json
/// ```
const TARGET: RustTarget = RustTarget::SpecJson {
    name: "riscv64ima-unknown-none-elf",
    json: include_str!("./rust_rv64ima/riscv64ima-unknown-none-elf.json"),
};

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
    // For using json target spec
    "-Zjson-target-spec",
];

/// Copied from https://github.com/0xPolygonHermez/rust/blob/c03068e/compiler/rustc_target/src/spec/targets/riscv64ima_zisk_zkvm_elf_linker_script.ld.
const LINKER_SCRIPT: &str = include_str!("rust_rv64ima/link.x");

/// Compiler for Rust guest program to RV64IMA architecture, using a stock
/// nightly Rust toolchain with ZisK's target specification.
pub struct RustRv64ima;

impl Compiler for RustRv64ima {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .linker_script(Some(LINKER_SCRIPT))
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET)?;
        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv64ima, prover::ZiskProver};
    use ere_prover_core::{
        Input,
        compiler::Compiler,
        prover::{ProverResource, zkVMProver},
    };
    use ere_util_test::host::testing_guest_directory;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("zisk", "stock_nightly_no_std");
        let elf = RustRv64ima.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("zisk", "stock_nightly_no_std");
        let elf = RustRv64ima.compile(guest_directory).unwrap();
        let zkvm = ZiskProver::new(elf, ProverResource::Cpu).unwrap();

        zkvm.execute(&Input::new()).unwrap();
    }
}

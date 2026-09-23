use std::{env, path::Path};

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CargoBuildCmd, RustTarget, parse_cargo_build_options};

use crate::Error;

/// Target spec modified from `riscv64im-unknown-none-elf` with patch `atomic-cas = true`.
///
/// To reproduce:
///
/// ```bash
/// rustc +nightly -Z unstable-options --print target-spec-json --target riscv64im-unknown-none-elf \
///     | jq '.["atomic-cas"] = true' \
///     > ./crates/compiler/lambdavm/src/rust_rv64ima/riscv64ima-unknown-none-elf.json
/// ```
const TARGET: RustTarget = RustTarget::SpecJson {
    name: "riscv64ima-unknown-none-elf",
    json: include_str!("./rust_rv64ima/riscv64ima-unknown-none-elf.json"),
};

const RUSTFLAGS: &[&str] = &[
    // Replace atomic ops with nonatomic versions since the guest is single threaded.
    "-C",
    "passes=lower-atomic",
    "-C",
    "panic=abort",
    // https://docs.rs/getrandom/0.3.2/getrandom/index.html#opt-in-backends
    "--cfg",
    "getrandom_backend=\"custom\"",
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
    // Take `memcpy` and friends from `compiler_builtins` when building the
    // standard library crates from source.
    "-Zbuild-std-features=compiler-builtins-mem",
    // For using json target spec
    "-Zjson-target-spec",
];

/// Compiler for Rust guest program to RV64IMA architecture.
pub struct LambdaVMRustRv64ima;

impl Compiler for LambdaVMRustRv64ima {
    type Error = Error;

    fn compile(
        &self,
        guest_directory: impl AsRef<Path>,
        args: &[String],
    ) -> Result<Elf, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let options = parse_cargo_build_options(args)?;
        let elf = CargoBuildCmd::new()
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .features(&options.features)
            .ignore_rust_version(options.ignore_rust_version)
            .exec(guest_directory, TARGET)?;
        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use ere_compiler_core::Compiler;
    use ere_prover_core::{Input, ProverResource, zkVMProver};
    use ere_prover_lambdavm::LambdaVMProver;
    use ere_util_test::host::testing_guest_directory;

    use crate::LambdaVMRustRv64ima;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("lambdavm", "stock_nightly_no_std");
        let elf = LambdaVMRustRv64ima.compile(guest_directory, &[]).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("lambdavm", "stock_nightly_no_std");
        let elf = LambdaVMRustRv64ima.compile(guest_directory, &[]).unwrap();
        let zkvm = LambdaVMProver::new(elf, ProverResource::Cpu).unwrap();
        zkvm.execute(&Input::new()).unwrap();
    }
}

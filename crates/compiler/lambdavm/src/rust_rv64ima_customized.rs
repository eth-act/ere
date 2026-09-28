use std::path::Path;

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CargoBuildCmd, RustTarget, parse_cargo_build_options};

use crate::Error;

/// According to https://github.com/yetanotherco/lambda_vm/blob/v0.1.0/Makefile#L201.
const LAMBDAVM_TOOLCHAIN: &str = "nightly-2026-02-01";

/// Target spec of LambdaVM, copied verbatim.
///
/// According to https://github.com/yetanotherco/lambda_vm/blob/v0.1.0/executor/programs/riscv64im-lambda-vm-elf.json.
const TARGET: RustTarget = RustTarget::SpecJson {
    name: "riscv64im-lambda-vm-elf",
    json: include_str!("./rust_rv64ima_customized/riscv64im-lambda-vm-elf.json"),
};

/// Rust flags according to https://github.com/yetanotherco/lambda_vm/blob/v0.1.0/executor/programs/rust/panic/.cargo/config.toml
const RUSTFLAGS: &[&str] = &[
    // https://docs.rs/getrandom/0.3.2/getrandom/index.html#opt-in-backends
    "--cfg",
    "getrandom_backend=\"custom\"",
    // Replace atomic ops with nonatomic versions since the guest is single threaded.
    "-C",
    "passes=lower-atomic",
];
/// Cargo build options according to https://github.com/yetanotherco/lambda_vm/blob/v0.1.0/Makefile#L197-L209
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // The target has no prebuilt standard library, so build it with `std`
    "-Zbuild-std=core,alloc,std,compiler_builtins,panic_abort",
    // Take `memcpy` and friends from `compiler_builtins` when building the
    // standard library crates from source.
    "-Zbuild-std-features=compiler-builtins-mem",
    // For using json target spec
    "-Zjson-target-spec",
];

/// Compiler for Rust guest program to RV64IMA architecture, using the Rust
/// toolchain pinned by LambdaVM and target `riscv64im-lambda-vm-elf`.
pub struct LambdaVMRustRv64imaCustomized;

impl Compiler for LambdaVMRustRv64imaCustomized {
    type Error = Error;

    fn compile(
        &self,
        guest_directory: impl AsRef<Path>,
        args: &[String],
    ) -> Result<Elf, Self::Error> {
        let options = parse_cargo_build_options(args)?;
        let elf = CargoBuildCmd::new()
            .toolchain(LAMBDAVM_TOOLCHAIN)
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
    use ere_util_test::host::testing_guest_directory;

    use crate::LambdaVMRustRv64imaCustomized;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("lambdavm", "basic");
        let elf = LambdaVMRustRv64imaCustomized
            .compile(guest_directory, &[])
            .unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

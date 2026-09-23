use std::path::Path;

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CargoBuildCmd, RustTarget, parse_cargo_build_options};

use crate::Error;

/// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/Makefile#L201.
const LAMBDAVM_TOOLCHAIN: &str = "nightly-2026-02-01";

/// Target spec of LambdaVM, copied verbatim.
///
/// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/executor/programs/riscv64im-lambda-vm-elf.json.
const TARGET: RustTarget = RustTarget::SpecJson {
    name: "riscv64im-lambda-vm-elf",
    json: include_str!("./rust_rv64ima_customized/riscv64im-lambda-vm-elf.json"),
};

/// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/executor/programs/rust/panic/.cargo/config.toml.
const RUSTFLAGS: &[&str] = &[
    "--cfg",
    "getrandom_backend=\"custom\"",
    "-C",
    "passes=lower-atomic",
];

/// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/Makefile#L197-L209.
const CARGO_BUILD_OPTIONS: &[&str] = &[
    "-Zbuild-std=core,alloc,std,compiler_builtins,panic_abort",
    "-Zbuild-std-features=compiler-builtins-mem",
    // For using json target spec
    "-Zjson-target-spec",
];

/// Compiler for Rust guest program to RV64IMA architecture, using the target spec and nightly
/// toolchain of LambdaVM.
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

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
///     > ./crates/compiler/pico/src/rust_rv64ima/riscv64ima-unknown-none-elf.json
/// ```
const TARGET: RustTarget = RustTarget::SpecJson {
    name: "riscv64ima-unknown-none-elf",
    json: include_str!("./rust_rv64ima/riscv64ima-unknown-none-elf.json"),
};

/// Rust flags according to https://github.com/brevis-network/pico/blob/613d6aeb64350d73faf24c17e2e3e3cee5b763bc/sdk/cli/src/build/build.rs#L130-L157
const RUSTFLAGS: &[&str] = &[
    // Replace atomic ops with nonatomic versions since the guest is single threaded.
    "-C",
    "passes=lower-atomic",
    // Pico loads the program at this address.
    "-C",
    "link-arg=-Ttext=0x00200800",
    // A missing entry point is only a linker warning otherwise.
    "-C",
    "link-arg=--fatal-warnings",
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
pub struct PicoRustRv64ima;

impl Compiler for PicoRustRv64ima {
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
    use ere_util_test::host::testing_guest_directory;

    use crate::PicoRustRv64ima;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("pico", "stock_nightly_no_std");
        let elf = PicoRustRv64ima.compile(guest_directory, &[]).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

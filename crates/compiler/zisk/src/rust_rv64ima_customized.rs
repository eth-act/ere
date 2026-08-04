use std::path::Path;

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CargoBuildCmd, parse_cargo_build_options};

use crate::Error;

const ZISK_TOOLCHAIN: &str = "zisk";
const ZISK_TARGET: &str = "riscv64ima-zisk-zkvm-elf";

const RUSTFLAGS: &[&str] = &["-C", "passes=lower-atomic"];

/// LLVM flags tuned by Optuna for the Ethrex guest on Zisk
const PROFILE_ETHREX: &[&str] = &[
    "-C",
    "llvm-args=--inline-threshold=4749",
    "-C",
    "llvm-args=--unroll-threshold=378",
    "-C",
    "llvm-args=--memdep-block-number-limit=2510",
    "-C",
    "llvm-args=--memdep-block-scan-limit=98",
    "-C",
    "llvm-args=--jump-threading-threshold=16",
    "-C",
    "llvm-args=--max-speculation-depth=0",
    "-C",
    "llvm-args=--licm-versioning-max-depth-threshold=0",
    "-C",
    "llvm-args=--max-uses-for-sinking=119",
    "-C",
    "llvm-args=--inline-instr-cost=1",
    "-C",
    "llvm-args=--inline-memaccess-cost=1",
    "-C",
    "llvm-args=--inline-call-penalty=12",
    "-C",
    "llvm-args=--available-load-scan-limit=23",
    "-C",
    "llvm-args=--bonus-inst-threshold=5",
    "-C",
    "llvm-args=--max-num-inline-blocks=6",
    "-C",
    "llvm-args=--loop-interchange-threshold=2",
    "-C",
    "llvm-args=--licm-max-num-uses-traversed=24",
    "-C",
    "llvm-args=--early-ifcvt-limit=29",
    "-C",
    "llvm-args=--jump-threading-implication-search-threshold=6",
    "-C",
    "llvm-args=--loop-distribute-scev-check-threshold=0",
    "-C",
    "llvm-args=--loop-load-elimination-scev-check-threshold=7",
    "-C",
    "llvm-args=--max-dependences=6",
    "-C",
    "llvm-args=--max-nested-scalar-reduction-interleave=2",
    "-C",
    "llvm-args=--disable-licm-promotion",
];

fn profile_flags() -> &'static [&'static str] {
    match std::env::var("ERE_PROFILE").as_deref() {
        Ok("ethrex") => PROFILE_ETHREX,
        _ => &[],
    }
}

/// Compiler for Rust guest program to RV64IMA architecture, using customized
/// Rust toolchain of ZisK.
pub struct ZiskRustRv64imaCustomized;

impl Compiler for ZiskRustRv64imaCustomized {
    type Error = Error;

    fn compile(
        &self,
        guest_directory: impl AsRef<Path>,
        args: &[String],
    ) -> Result<Elf, Self::Error> {
        let flags: Vec<&str> = [profile_flags(), RUSTFLAGS].concat();
        let options = parse_cargo_build_options(args)?;
        let elf = CargoBuildCmd::new()
            .toolchain(ZISK_TOOLCHAIN)
            .rustflags(&flags)
            .features(&options.features)
            .ignore_rust_version(options.ignore_rust_version)
            .exec(guest_directory, ZISK_TARGET)?;
        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    use crate::ZiskRustRv64imaCustomized;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("zisk", "basic_rust");
        let elf = ZiskRustRv64imaCustomized
            .compile(guest_directory, &[])
            .unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

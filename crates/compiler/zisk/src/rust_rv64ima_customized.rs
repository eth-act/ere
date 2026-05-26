use std::path::Path;

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CargoBuildCmd, parse_cargo_features};

use crate::Error;

const ZISK_TOOLCHAIN: &str = "zisk";
const ZISK_TARGET: &str = "riscv64ima-zisk-zkvm-elf";

const RUSTFLAGS: &[&str] = &["-C", "passes=lower-atomic"];

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
        let elf = CargoBuildCmd::new()
            .toolchain(ZISK_TOOLCHAIN)
            .rustflags(RUSTFLAGS)
            .features(&parse_cargo_features(args)?)
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

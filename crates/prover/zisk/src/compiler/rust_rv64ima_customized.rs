use crate::compiler::Error;
use ere_prover_core::compiler::{Compiler, Elf};
use ere_util_compile::{CommonError, cargo_metadata, rustc_path};
use std::{fs, path::Path, process::Command};
use tracing::info;

const ZISK_TOOLCHAIN: &str = "zisk";
const ZISK_TARGET: &str = "riscv64ima-zisk-zkvm-elf";

/// Compiler for Rust guest program to RV64IMA architecture, using customized
/// Rust toolchain of ZisK.
pub struct RustRv64imaCustomized;

impl Compiler for RustRv64imaCustomized {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let guest_directory = guest_directory.as_ref();
        info!(
            "Compiling Rust ZisK program at {}",
            guest_directory.display()
        );

        let metadata = cargo_metadata(guest_directory)?;
        let package = metadata.root_package().unwrap();

        info!("Parsed program name: {}", package.name);

        let mut cmd = Command::new("cargo");
        let status = cmd
            .env("RUSTC", rustc_path(ZISK_TOOLCHAIN)?)
            .args(["build", "--release"])
            .args(["--target", ZISK_TARGET])
            .arg("--manifest-path")
            .arg(&package.manifest_path)
            .status()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !status.success() {
            return Err(CommonError::command_exit_non_zero(&cmd, status, None))?;
        }

        let elf_path = metadata
            .target_directory
            .join("riscv64ima-zisk-zkvm-elf")
            .join("release")
            .join(&package.name);
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", elf_path, err))?;

        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv64imaCustomized;
    use ere_prover_core::compiler::Compiler;
    use ere_util_test::host::testing_guest_directory;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("zisk", "basic_rust");
        let elf = RustRv64imaCustomized.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

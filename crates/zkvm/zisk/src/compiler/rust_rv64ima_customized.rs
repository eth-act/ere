use crate::error::CompileError;
use ere_compile_utils::{CommonError, cargo_metadata, rustc_path};
use ere_zkvm_interface::Compiler;
use std::{fs, path::Path, process::Command};
use tracing::info;

const ZISK_TOOLCHAIN: &str = "zisk";
const ZISK_TARGET: &str = "riscv64ima-zisk-zkvm-elf";

/// Compiler for Rust guest program to RV64IMA architecture, using customized
/// Rust toolchain of ZisK.
pub struct RustRv64imaCustomized;

impl Compiler for RustRv64imaCustomized {
    type Error = CompileError;

    type Program = Vec<u8>;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        info!("Compiling ZisK program at {}", guest_directory.display());

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
        let elf_bytes =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", elf_path, err))?;

        Ok(elf_bytes)
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv64imaCustomized;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("zisk", "basic");
        let elf_bytes = RustRv64imaCustomized.compile(&guest_directory).unwrap();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }
}

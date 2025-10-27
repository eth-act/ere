use crate::error::CompileError;
use ere_compile_utils::{CommonError, cargo_metadata};
use ere_zkvm_interface::Compiler;
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// Rust toolchain of Pico.
pub struct RustRv32imaCustomized;

impl Compiler for RustRv32imaCustomized {
    type Error = CompileError;

    type Program = Vec<u8>;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let tempdir = tempdir().map_err(CommonError::tempdir)?;

        cargo_metadata(guest_directory)?;

        let mut cmd = Command::new("cargo");
        let status = cmd
            .current_dir(guest_directory)
            .env("RUST_LOG", "info")
            .args(["pico", "build", "--output-directory"])
            .arg(tempdir.path())
            .status()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !status.success() {
            return Err(CommonError::command_exit_non_zero(&cmd, status, None))?;
        }

        let elf_path = tempdir.path().join("riscv32im-pico-zkvm-elf");
        let elf_bytes =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;

        Ok(elf_bytes)
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv32imaCustomized;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("pico", "basic");
        let elf = RustRv32imaCustomized.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

use crate::error::{CompileError, PicoError};
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;
use zkvm_interface::Compiler;

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// Rust toolchain of Pico.
pub struct RustRv32imaCustomized;

impl Compiler for RustRv32imaCustomized {
    type Error = PicoError;

    type Program = Vec<u8>;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let tempdir = tempdir().map_err(CompileError::Tempdir)?;

        // 1. Check guest path
        if !guest_directory.exists() {
            return Err(CompileError::PathNotFound(guest_directory.to_path_buf()))?;
        }

        // 2. Run `cargo pico build`
        let status = Command::new("cargo")
            .current_dir(guest_directory)
            .env("RUST_LOG", "info")
            .args(["pico", "build", "--output-directory"])
            .arg(tempdir.path())
            .status()
            .map_err(CompileError::CargoPicoBuild)?;

        if !status.success() {
            return Err(CompileError::CargoPicoBuildFailed { status })?;
        }

        // 3. Locate the ELF file
        let elf_path = tempdir.path().join("riscv32im-pico-zkvm-elf");

        if !elf_path.exists() {
            return Err(CompileError::ElfNotFound(elf_path))?;
        }

        // 4. Read the ELF file
        let elf_bytes = fs::read(&elf_path).map_err(|e| CompileError::ReadElf {
            path: elf_path,
            source: e,
        })?;

        Ok(elf_bytes)
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv32imaCustomized;
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("pico", "basic");
        let elf = RustRv32imaCustomized.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

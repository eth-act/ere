use crate::{compiler::SP1Program, error::CompileError};
use ere_compile_utils::{CommonError, cargo_metadata};
use ere_zkvm_interface::Compiler;
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;
use tracing::info;

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// Rust toolchain of Succinct.
pub struct RustRv32imaCustomized;

impl Compiler for RustRv32imaCustomized {
    type Error = CompileError;

    type Program = SP1Program;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        info!("Compiling SP1 program at {}", guest_directory.display());

        cargo_metadata(guest_directory)?;

        // ── build into a temp dir ─────────────────────────────────────────────
        let output_dir = tempdir().map_err(CommonError::tempdir)?;

        info!(
            "Running `cargo prove build` → dir: {}",
            output_dir.path().display(),
        );

        let mut cmd = Command::new("cargo");
        let status = cmd
            .current_dir(guest_directory)
            .args([
                "prove",
                "build",
                "--output-directory",
                &output_dir.path().to_string_lossy(),
                "--elf-name",
                "guest.elf",
            ])
            .status()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !status.success() {
            return Err(CommonError::command_exit_non_zero(&cmd, status, None))?;
        }

        let elf_path = output_dir.path().join("guest.elf");
        let elf_bytes =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;
        info!("SP1 program compiled OK - {} bytes", elf_bytes.len());

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
        let guest_directory = testing_guest_directory("sp1", "basic");
        let elf_bytes = RustRv32imaCustomized.compile(&guest_directory).unwrap();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }
}

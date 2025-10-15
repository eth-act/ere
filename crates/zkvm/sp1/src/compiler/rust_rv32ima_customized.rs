use crate::{
    compiler::SP1Program,
    error::{CompileError, SP1Error},
};
use ere_zkvm_interface::Compiler;
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;
use tracing::info;

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// Rust toolchain of Succinct.
pub struct RustRv32imaCustomized;

impl Compiler for RustRv32imaCustomized {
    type Error = SP1Error;

    type Program = SP1Program;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        info!("Compiling SP1 program at {}", guest_directory.display());

        if !guest_directory.exists() || !guest_directory.is_dir() {
            return Err(CompileError::InvalidProgramPath(
                guest_directory.to_path_buf(),
            ))?;
        }

        let guest_manifest_path = guest_directory.join("Cargo.toml");
        if !guest_manifest_path.exists() {
            return Err(CompileError::CargoTomlMissing {
                program_dir: guest_directory.to_path_buf(),
                manifest_path: guest_manifest_path.clone(),
            })?;
        }

        // ── build into a temp dir ─────────────────────────────────────────────
        let temp_output_dir = tempdir().map_err(CompileError::TempDir)?;
        let temp_output_dir_path = temp_output_dir.path();

        info!(
            "Running `cargo prove build` → dir: {}",
            temp_output_dir_path.display(),
        );

        let status = Command::new("cargo")
            .current_dir(guest_directory)
            .args([
                "prove",
                "build",
                "--output-directory",
                temp_output_dir_path.to_str().unwrap(),
                "--elf-name",
                "guest.elf",
            ])
            .status()
            .map_err(|e| CompileError::CargoProveBuild {
                cwd: guest_directory.to_path_buf(),
                source: e,
            })?;

        if !status.success() {
            return Err(CompileError::CargoProveBuildFailed {
                status,
                path: guest_directory.to_path_buf(),
            })?;
        }

        let elf_path = temp_output_dir_path.join("guest.elf");
        if !elf_path.exists() {
            return Err(CompileError::ElfNotFound(elf_path))?;
        }

        let elf_bytes = fs::read(&elf_path).map_err(|e| CompileError::ReadFile {
            path: elf_path,
            source: e,
        })?;
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

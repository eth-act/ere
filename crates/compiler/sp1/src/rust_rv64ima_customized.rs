use crate::Error;
use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CommonError, cargo_metadata};
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;
use tracing::info;

/// Compiler for Rust guest program to RV64IMA architecture, using customized
/// Rust toolchain of Succinct.
pub struct SP1RustRv64imaCustomized;

impl Compiler for SP1RustRv64imaCustomized {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let guest_directory = guest_directory.as_ref();
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
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;
        info!("SP1 program compiled OK - {} bytes", elf.len());

        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use crate::SP1RustRv64imaCustomized;
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("sp1", "basic");
        let elf = SP1RustRv64imaCustomized.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

use std::{fs, path::Path, process::Command};

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CommonError, cargo_metadata, parse_cargo_build_options};
use tempfile::tempdir;
use tracing::info;

use crate::Error;

/// Compiler for Rust guest program to RV64IMA architecture, using `cargo pico build` with the
/// `pico` toolchain and target `riscv64im-pico-zkvm-elf`.
///
/// Building through `cargo pico` keeps the ELF, and so the program identity, the same as a Pico
/// user's own build.
pub struct PicoRustRv64imaCustomized;

impl Compiler for PicoRustRv64imaCustomized {
    type Error = Error;

    fn compile(
        &self,
        guest_directory: impl AsRef<Path>,
        args: &[String],
    ) -> Result<Elf, Self::Error> {
        let guest_directory = guest_directory.as_ref();
        info!("Compiling Pico program at {}", guest_directory.display());

        cargo_metadata(guest_directory)?;

        let output_dir = tempdir().map_err(CommonError::tempdir)?;

        let mut cmd = Command::new("cargo");
        cmd.current_dir(guest_directory).args([
            "pico",
            "build",
            "--output-directory",
            &output_dir.path().to_string_lossy(),
            "--elf-name",
            "guest.elf",
        ]);
        let options = parse_cargo_build_options(args)?;
        if !options.features.is_empty() {
            cmd.args(["--features", &options.features.join(",")]);
        }
        if options.ignore_rust_version {
            cmd.arg("--ignore-rust-version");
        }
        let status = cmd
            .status()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !status.success() {
            Err(CommonError::command_exit_non_zero(&cmd, status, None))?;
        }

        let elf_path = output_dir.path().join("guest.elf");
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;

        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    use crate::PicoRustRv64imaCustomized;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("pico", "basic");
        let elf = PicoRustRv64imaCustomized
            .compile(guest_directory, &[])
            .unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

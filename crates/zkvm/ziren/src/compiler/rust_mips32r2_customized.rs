use crate::{compiler::Error, program::ZirenProgram};
use ere_compile_utils::{CommonError, cargo_metadata, rustc_path};
use ere_zkvm_interface::compiler::Compiler;
use std::{fs, path::Path, process::Command};

const ZKM_TOOLCHAIN: &str = "zkm";

/// Compiler for Rust guest program to MIPS32R2 architecture, using customized
/// Rust toolchain of ZKM.
pub struct RustMips32r2Customized;

impl Compiler for RustMips32r2Customized {
    type Error = Error;

    type Program = ZirenProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let metadata = cargo_metadata(guest_directory)?;
        let package = metadata.root_package().unwrap();

        // Use `cargo ziren build` instead of using crate `zkm-build`, because
        // it exits if the underlying `cargo build` fails, and there is no way
        // to recover.
        let mut cmd = Command::new("cargo");
        let output = cmd
            .current_dir(guest_directory)
            .env("RUSTUP_TOOLCHAIN", "nightly")
            .env("RUSTC", rustc_path(ZKM_TOOLCHAIN)?)
            .env("ZIREN_ZKM_CC", "mipsel-zkm-zkvm-elf-gcc")
            .args(["ziren", "build"])
            .output()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !output.status.success() {
            return Err(CommonError::command_exit_non_zero(
                &cmd,
                output.status,
                Some(&output),
            ))?;
        }

        let elf_path = metadata
            .target_directory
            .join("elf-compilation")
            .join("mipsel-zkm-zkvm-elf")
            .join("release")
            .join(&package.name);
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;

        Ok(ZirenProgram { elf })
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustMips32r2Customized;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::compiler::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("ziren", "basic");
        let program = RustMips32r2Customized.compile(&guest_directory).unwrap();
        assert!(!program.elf().is_empty(), "ELF bytes should not be empty.");
    }
}

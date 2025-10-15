use crate::{
    compiler::ZirenProgram,
    error::{CompileError, ZirenError},
};
use compile_utils::cargo_metadata;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use zkvm_interface::Compiler;

const ZKM_TOOLCHAIN: &str = "zkm";

/// Compiler for Rust guest program to MIPS32R2 architecture, using customized
/// Rust toolchain of ZKM.
pub struct RustMips32r2Customized;

impl Compiler for RustMips32r2Customized {
    type Error = ZirenError;

    type Program = ZirenProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let metadata = cargo_metadata(guest_directory).map_err(CompileError::CompileUtilError)?;
        let package = metadata.root_package().unwrap();

        let rustc = {
            let output = Command::new("rustc")
                .env("RUSTUP_TOOLCHAIN", ZKM_TOOLCHAIN)
                .args(["--print", "sysroot"])
                .output()
                .map_err(CompileError::RustcSysrootFailed)?;

            if !output.status.success() {
                return Err(CompileError::RustcSysrootExitNonZero {
                    status: output.status,
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                })?;
            }

            PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
                .join("bin")
                .join("rustc")
        };

        // Use `cargo ziren build` instead of using crate `zkm-build`, because
        // it exits if the underlying `cargo build` fails, and there is no way
        // to recover.
        let output = Command::new("cargo")
            .current_dir(guest_directory)
            .env("RUSTC", rustc)
            .env("ZIREN_ZKM_CC", "mipsel-zkm-zkvm-elf-gcc")
            .args(["ziren", "build"])
            .output()
            .map_err(CompileError::CargoZirenBuildFailed)?;

        if !output.status.success() {
            return Err(CompileError::CargoZirenBuildExitNonZero {
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            })?;
        }

        let elf_path = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| {
                let line = line.strip_prefix("cargo:rustc-env=ZKM_ELF_")?;
                let (package_name, elf_path) = line.split_once("=")?;
                (package_name == package.name).then(|| PathBuf::from(elf_path))
            })
            .ok_or_else(|| CompileError::GuestNotFound {
                name: package.name.clone(),
            })?;

        let elf = fs::read(&elf_path).map_err(|source| CompileError::ReadFile {
            path: elf_path,
            source,
        })?;

        Ok(elf)
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustMips32r2Customized;
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("ziren", "basic");
        let elf_bytes = RustMips32r2Customized.compile(&guest_directory).unwrap();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }
}

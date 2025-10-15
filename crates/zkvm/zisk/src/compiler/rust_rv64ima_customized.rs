use crate::error::ZiskError;
use ere_compile_utils::cargo_metadata;
use ere_zkvm_interface::Compiler;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::info;

const ZISK_TOOLCHAIN: &str = "zisk";
const ZISK_TARGET: &str = "riscv64ima-zisk-zkvm-elf";

/// Compiler for Rust guest program to RV64IMA architecture, using customized
/// Rust toolchain of ZisK.
pub struct RustRv64imaCustomized;

impl Compiler for RustRv64imaCustomized {
    type Error = ZiskError;

    type Program = Vec<u8>;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        info!("Compiling ZisK program at {}", guest_directory.display());

        let metadata = cargo_metadata(guest_directory)?;
        let package_name = &metadata.root_package().unwrap().name;

        info!("Parsed program name: {package_name}");

        // ── build ─────────────────────────────────────────────────────────────
        // Get the path to ZisK toolchain's `rustc` so we could set the env
        // `RUSTC=...` for `cargo` instead of using `cargo +zisk ...`.
        let zisk_rustc = {
            let output = Command::new("rustc")
                .env("RUSTUP_TOOLCHAIN", ZISK_TOOLCHAIN)
                .arg("--print")
                .arg("sysroot")
                .output()
                .map_err(ZiskError::RustcSysroot)?;
            PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
                .join("bin")
                .join("rustc")
        };

        let status = Command::new("cargo")
            .current_dir(guest_directory)
            .env("RUSTC", zisk_rustc)
            .args(["build", "--release", "--target", ZISK_TARGET])
            .status()
            .map_err(|e| ZiskError::CargoBuild {
                cwd: guest_directory.to_path_buf(),
                source: e,
            })?;

        if !status.success() {
            return Err(ZiskError::CargoBuildFailed {
                status,
                path: guest_directory.to_path_buf(),
            });
        }

        // Get the workspace directory.
        let program_workspace_path = {
            let output = Command::new("cargo")
                .current_dir(guest_directory)
                .arg("locate-project")
                .arg("--workspace")
                .arg("--message-format=plain")
                .output()
                .map_err(ZiskError::CargoLocateProject)?;
            PathBuf::from(
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .strip_suffix("Cargo.toml")
                    .expect("location to be path to Cargo.toml"),
            )
        };

        let elf_path = program_workspace_path
            .join("target")
            .join("riscv64ima-zisk-zkvm-elf")
            .join("release")
            .join(package_name);
        let elf_bytes = fs::read(&elf_path).map_err(|e| ZiskError::ReadFile {
            path: elf_path.clone(),
            source: e,
        })?;

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

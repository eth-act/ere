use crate::error::CompileError;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use toml::Value as TomlValue;
use tracing::info;

const ZISK_TOOLCHAIN: &str = "zisk";
const ZISK_TARGET: &str = "riscv64ima-zisk-zkvm-elf";

/// Compile the guest crate and return raw ELF bytes.
pub fn compile_zisk_program(program_crate_path: &Path) -> Result<Vec<u8>, CompileError> {
    info!("Compiling ZisK program at {}", program_crate_path.display());

    if !program_crate_path.exists() || !program_crate_path.is_dir() {
        return Err(CompileError::InvalidProgramPath(
            program_crate_path.to_path_buf(),
        ));
    }

    let guest_manifest_path = program_crate_path.join("Cargo.toml");
    if !guest_manifest_path.exists() {
        return Err(CompileError::CargoTomlMissing {
            program_dir: program_crate_path.to_path_buf(),
            manifest_path: guest_manifest_path.clone(),
        });
    }

    // ── read + parse Cargo.toml ───────────────────────────────────────────
    let manifest_content =
        fs::read_to_string(&guest_manifest_path).map_err(|e| CompileError::ReadFile {
            path: guest_manifest_path.clone(),
            source: e,
        })?;

    let manifest_toml: TomlValue =
        manifest_content
            .parse::<TomlValue>()
            .map_err(|e| CompileError::ParseCargoToml {
                path: guest_manifest_path.clone(),
                source: e,
            })?;

    let program_name = manifest_toml
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| CompileError::MissingPackageName {
            path: guest_manifest_path.clone(),
        })?;

    info!("Parsed program name: {program_name}");

    // ── build ─────────────────────────────────────────────────────────────
    // Get the path to ZisK toolchain's `rustc` so we could set the env
    // `RUSTC=...` for `cargo` instead of using `cargo +zisk ...`.
    let zisk_rustc = {
        let output = Command::new("rustc")
            .env("RUSTUP_TOOLCHAIN", ZISK_TOOLCHAIN)
            .arg("--print")
            .arg("sysroot")
            .output()
            .map_err(|e| CompileError::RustcSysroot { source: e })?;
        PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
            .join("bin")
            .join("rustc")
    };

    let status = Command::new("cargo")
        .current_dir(program_crate_path)
        .env("RUSTC", zisk_rustc)
        .args(["build", "--release", "--target", ZISK_TARGET])
        .status()
        .map_err(|e| CompileError::CargoBuild {
            cwd: program_crate_path.to_path_buf(),
            source: e,
        })?;

    if !status.success() {
        return Err(CompileError::CargoBuildFailed {
            status,
            path: program_crate_path.to_path_buf(),
        });
    }

    // Get the workspace directory.
    let program_workspace_path = {
        let output = Command::new("cargo")
            .current_dir(program_crate_path)
            .arg("locate-project")
            .arg("--workspace")
            .arg("--message-format=plain")
            .output()
            .map_err(|e| CompileError::CargoLocateProject { source: e })?;
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
        .join(program_name);
    let elf_bytes = fs::read(&elf_path).map_err(|e| CompileError::ReadFile {
        path: elf_path.clone(),
        source: e,
    })?;

    Ok(elf_bytes)
}

#[cfg(test)]
mod tests {
    use zkvm_interface::Compiler;

    use crate::RV64_IMA_ZISK_ZKVM_ELF;

    use super::*;
    use std::path::PathBuf;

    // TODO: for now, we just get one test file
    // TODO: but this should get the whole directory and compile each test
    fn get_compile_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("zisk")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/zisk")
    }

    #[test]
    fn test_compile_zisk_program() {
        let test_guest_path = get_compile_test_guest_program_path();

        match compile_zisk_program(&test_guest_path) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(e) => {
                panic!("compile failed for dedicated guest: {e:?}");
            }
        }
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        match RV64_IMA_ZISK_ZKVM_ELF::compile(&test_guest_path, Path::new("")) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(e) => {
                panic!("compile_zisk_program direct call failed for dedicated guest: {e:?}");
            }
        }
    }
}

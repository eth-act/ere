use crate::error::CompileError;
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;
use tracing::info;

pub fn compile(guest_directory: &Path) -> Result<Vec<u8>, CompileError> {
    info!("Compiling SP1 program at {}", guest_directory.display());

    if !guest_directory.exists() || !guest_directory.is_dir() {
        return Err(CompileError::InvalidProgramPath(
            guest_directory.to_path_buf(),
        ));
    }

    let guest_manifest_path = guest_directory.join("Cargo.toml");
    if !guest_manifest_path.exists() {
        return Err(CompileError::CargoTomlMissing {
            program_dir: guest_directory.to_path_buf(),
            manifest_path: guest_manifest_path.clone(),
        });
    }

    // ── read + parse Cargo.toml ───────────────────────────────────────────
    let manifest_content =
        fs::read_to_string(&guest_manifest_path).map_err(|e| CompileError::ReadFile {
            path: guest_manifest_path.clone(),
            source: e,
        })?;

    let manifest_toml: toml::Value =
        manifest_content
            .parse::<toml::Value>()
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

    // ── build into a temp dir ─────────────────────────────────────────────
    let temp_output_dir = TempDir::new_in(guest_directory)?;
    let temp_output_dir_path = temp_output_dir.path();
    let elf_name = format!("{program_name}.elf");

    info!(
        "Running `cargo prove build` → dir: {}, ELF: {}",
        temp_output_dir_path.display(),
        elf_name
    );

    let status = Command::new("cargo")
        .current_dir(guest_directory)
        .args([
            "prove",
            "build",
            "--output-directory",
            temp_output_dir_path.to_str().unwrap(),
            "--elf-name",
            &elf_name,
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| CompileError::CargoProveBuild {
            cwd: guest_directory.to_path_buf(),
            source: e,
        })?;

    if !status.success() {
        return Err(CompileError::CargoBuildFailed {
            status,
            path: guest_directory.to_path_buf(),
        });
    }

    let elf_path = temp_output_dir_path.join(&elf_name);
    if !elf_path.exists() {
        return Err(CompileError::ElfNotFound(elf_path));
    }

    let elf_bytes = fs::read(&elf_path).map_err(|e| CompileError::ReadFile {
        path: elf_path,
        source: e,
    })?;

    info!("SP1 program compiled OK - {} bytes", elf_bytes.len());
    Ok(elf_bytes)
}

#[cfg(test)]
mod tests {
    use zkvm_interface::Compiler;

    use crate::RV32_IM_SUCCINCT_ZKVM_ELF;

    use super::*;
    use std::path::PathBuf;

    // TODO: for now, we just get one test file
    // TODO: but this should get the whole directory and compile each test
    fn get_compile_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("sp1")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/sp1")
    }

    #[test]
    fn test_compile_sp1_program() {
        let test_guest_path = get_compile_test_guest_program_path();

        match compile(&test_guest_path) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(err) => {
                panic!("compile failed for dedicated guest: {err}");
            }
        }
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        match RV32_IM_SUCCINCT_ZKVM_ELF.compile(&test_guest_path) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(err) => {
                panic!("compile_sp1_program direct call failed for dedicated guest: {err}");
            }
        }
    }
}

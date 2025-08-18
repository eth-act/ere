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

    let elf_path = temp_output_dir_path.join("guest.elf");
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
    use crate::RV32_IM_SUCCINCT_ZKVM_ELF;
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::Compiler;

    #[test]
    fn test_compiler_impl() {
        let guest_directory = testing_guest_directory("sp1", "basic");
        let elf_bytes = RV32_IM_SUCCINCT_ZKVM_ELF.compile(&guest_directory).unwrap();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }
}

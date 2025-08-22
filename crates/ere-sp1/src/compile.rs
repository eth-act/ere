use crate::compile_stock_rust::stock_rust_compile;
use crate::error::CompileError;
use std::process::ExitStatus;
use std::{fs, path::Path, path::PathBuf, process::Command};
use tempfile::TempDir;
use tracing::info;

fn get_guest_program_name(guest_directory: &Path) -> Result<String, CompileError> {
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

    Ok(program_name.into())
}

fn sp1_compile(
    guest_directory: &Path,
    output_directory: &Path,
) -> Result<(ExitStatus, PathBuf), CompileError> {
    info!(
        "Running `cargo prove build` → dir: {}",
        output_directory.display(),
    );

    let result = Command::new("cargo")
        .current_dir(guest_directory)
        .args([
            "prove",
            "build",
            "--output-directory",
            output_directory.to_str().unwrap(),
            "--elf-name",
            "guest.elf",
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| CompileError::CargoProveBuild {
            cwd: guest_directory.to_path_buf(),
            source: e,
        });
    match result {
        Ok(status) => Ok((status, output_directory.join("guest.elf"))),
        Err(err) => Err(err),
    }
}

pub fn compile(guest_directory: &Path, toolchain: &String) -> Result<Vec<u8>, CompileError> {
    let program_name = get_guest_program_name(guest_directory)?;
    info!("Parsed program name: {program_name}");

    // ── build into a temp dir ─────────────────────────────────────────────
    let temp_output_dir = TempDir::new_in(guest_directory)?;
    let temp_output_dir_path = temp_output_dir.path();

    let (status, elf_path) = match toolchain.as_str() {
        "succinct" => sp1_compile(guest_directory, temp_output_dir_path)?,
        _ => stock_rust_compile(
            guest_directory,
            temp_output_dir_path,
            &program_name,
            toolchain,
        )?,
    };

    if !status.success() {
        return Err(CompileError::CargoBuildFailed {
            status,
            path: guest_directory.to_path_buf(),
        });
    }

    if !elf_path.exists() {
        return Err(CompileError::ElfNotFound(elf_path));
    }

    let elf_bytes = fs::read(&elf_path).map_err(|e| CompileError::ReadFile {
        path: elf_path,
        source: e,
    })?;
    info!(
        "SP1 ({}) program compiled OK - {} bytes",
        toolchain,
        elf_bytes.len()
    );

    Ok(elf_bytes)
}

#[cfg(test)]
mod tests {
    use crate::RV32_IM_SUCCINCT_ZKVM_ELF;
    use crate::compile::compile;
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::Compiler;

    #[test]
    fn test_compiler_impl() {
        let guest_directory = testing_guest_directory("sp1", "basic");
        let elf_bytes = RV32_IM_SUCCINCT_ZKVM_ELF.compile(&guest_directory).unwrap();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }
    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory("sp1", "stock_nightly_no_std");
        let elf_bytes = compile(&guest_directory, &"nightly".to_string()).unwrap();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }
}

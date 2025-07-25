use crate::error::CompileError;
use hex::FromHex;
use risc0_zkvm::Digest;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risc0Program {
    // TODO: Seems like the risc0 compilation is also compiling
    // TODO: the analogous prover and verifying key
    pub(crate) elf: Vec<u8>,
    pub(crate) image_id: Digest,
}

pub fn compile_risc0_program(guest_folder: &Path) -> Result<Risc0Program, CompileError> {
    info!("Compiling Risc0 program at {}", guest_folder.display());

    if !guest_folder.is_dir() {
        return Err(CompileError::InvalidGuestPath(guest_folder.to_path_buf()));
    }

    info!("Running `cargo risczero build`");

    let output = Command::new("cargo")
        .current_dir(guest_folder)
        .args(["risczero", "build"])
        .output()
        .map_err(|err| CompileError::io(err, "Failed to run `cargo risczero build`"))?;

    if !output.status.success() {
        return Err(CompileError::CargoRisczeroBuildFailure {
            crate_path: guest_folder.to_path_buf(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let (image_id, elf_path) = {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .find(|line| line.starts_with("ImageID: "))
            .ok_or(CompileError::MissingImageIdAndElfPath)?;
        let (image_id, elf_path) = line
            .trim_start_matches("ImageID: ")
            .split_once(" - ")
            .ok_or(CompileError::MissingImageIdAndElfPath)?;
        (image_id.to_string(), PathBuf::from(elf_path))
    };

    if !elf_path.exists() {
        return Err(CompileError::InvalidElfPath(elf_path));
    }

    let elf = fs::read(&elf_path).map_err(|err| CompileError::io(err, "Failed to read elf"))?;

    info!("Risc0 program compiled OK - {} bytes", elf.len());
    info!("Image ID - {image_id}");

    let image_id =
        Digest::from_hex(&image_id).map_err(|_| CompileError::InvalidImageId(image_id))?;

    Ok(Risc0Program { elf, image_id })
}

#[cfg(test)]
mod tests {
    mod compile {
        use crate::compile::compile_risc0_program;
        use std::path::PathBuf;

        fn get_test_risc0_methods_crate_path() -> PathBuf {
            let workspace_dir = env!("CARGO_WORKSPACE_DIR");
            PathBuf::from(workspace_dir)
                .join("tests")
                .join("risc0")
                .join("compile")
                .join("basic")
                .canonicalize()
                .expect("Failed to find or canonicalize test Risc0 methods crate")
        }

        #[test]
        fn test_compile_risc0_method() {
            let test_methods_path = get_test_risc0_methods_crate_path();

            let program =
                compile_risc0_program(&test_methods_path).expect("risc0 compilation failed");
            assert!(
                !program.elf.is_empty(),
                "Risc0 ELF bytes should not be empty."
            );
        }
    }
}

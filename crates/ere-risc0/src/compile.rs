mod file_utils;
use file_utils::FileRestorer;
use risc0_zkvm::Digest;

use crate::error::CompileError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risc0Program {
    // TODO: Seems like the risc0 compilation is also compiling
    // TODO: the analogous prover and verifying key
    pub(crate) elf: Vec<u8>,
    pub(crate) image_id: Digest,
}

/// BUILD_SCRIPT_TEMPLATE that we will use to fetch the elf-path
/// TODO: We might be able to deterministically get the elf path
/// TODO: But note we also probably want the image id too, so not sure
/// TODO: we can remove this hack sometime soon.
const BUILD_SCRIPT_TEMPLATE: &str = include_str!("../build_script_template.rs");

pub(crate) fn compile_risc0_program(path: &Path) -> Result<Risc0Program, CompileError> {
    if !path.exists() || !path.is_dir() {
        return Err(CompileError::InvalidMethodsPath(path.to_path_buf()));
    }

    // Inject `build.rs`
    let build_rs_path = path.join("build.rs");
    let _restorer = FileRestorer::new(&build_rs_path)?;
    fs::write(&build_rs_path, BUILD_SCRIPT_TEMPLATE)
        .map_err(|e| CompileError::io(e, "writing template build.rs"))?;

    // Run `cargo build`
    let output = Command::new("cargo")
        .current_dir(path)
        .arg("build")
        .arg("--release")
        .output()
        .map_err(|e| CompileError::io(e, "spawning cargo build"))?;

    if !output.status.success() {
        return Err(CompileError::CargoBuildFailure {
            crate_path: path.to_path_buf(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    // Read guest info JSON
    let info_file = path.join("ere_guest_info.json");
    let info_text = fs::read_to_string(&info_file)
        .map_err(|e| CompileError::io(e, "reading ere_guest_info.json"))?;
    let info_json: JsonValue = serde_json::from_str(&info_text)
        .map_err(|e| CompileError::serde(e, "parsing ere_guest_info.json"))?;

    let elf_path = info_json["elf_path"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| CompileError::MissingJsonField {
            field: "elf_path",
            file: info_file.clone(),
        })?;
    let image_id_hex_str = info_json["image_id_hex"].as_str().unwrap();
    let image_id = hex::decode(image_id_hex_str).unwrap();
    let image_id = image_id.try_into().unwrap();

    // Return Program
    fs::read(&elf_path)
        .map_err(|e| CompileError::io(e, "reading ELF file"))
        .map(|elf| Risc0Program { elf, image_id })
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
                .join("project_structure_build")
                .canonicalize()
                .expect("Failed to find or canonicalize test Risc0 methods crate")
        }

        #[test]
        fn test_compile_risc0_method_with_custom_build_rs() {
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

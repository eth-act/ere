use std::{fs, path::Path, process::Command};

use tempfile::TempDir;
use toml::Value as TomlValue;
use tracing::info;

use crate::error::CompileError;

#[cfg(test)]
mod tests {
    use zkvm_interface::Compiler;

    use crate::RV32_IM_ZKM_ZKVM_ELF;

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
    fn test_compile_zkm_program() {
        let test_guest_path = get_compile_test_guest_program_path();

        match compile_zkm_program(&test_guest_path) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(e) => {
                panic!("compile failed for dedicated guest: {:?}", e);
            }
        }
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        match RV32_IM_ZKM_ZKVM_ELF.compile(&test_guest_path) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(e) => {
                panic!(
                    "compile_zkm_program direct call failed for dedicated guest: {:?}",
                    e
                );
            }
        }
    }
}

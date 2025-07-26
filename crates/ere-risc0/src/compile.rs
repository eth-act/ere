use crate::error::CompileError;
use cargo_metadata::MetadataCommand;
use risc0_build::GuestOptions;
use risc0_zkvm::Digest;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risc0Program {
    pub(crate) elf: Vec<u8>,
    pub(crate) image_id: Digest,
}

pub fn compile_risc0_program(guest_directory: &Path) -> Result<Risc0Program, CompileError> {
    info!("Compiling Risc0 program at {}", guest_directory.display());

    let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
    let package = metadata
        .root_package()
        .ok_or_else(|| CompileError::MissingPackageName {
            path: guest_directory.to_path_buf(),
        })?;

    let guest =
        risc0_build::build_package(package, &metadata.target_directory, GuestOptions::default())
            .map_err(|source| CompileError::Risc0BuildFailure {
                source,
                crate_path: guest_directory.to_path_buf(),
            })?
            .into_iter()
            .next()
            .ok_or(CompileError::Risc0BuildMissingGuest)?;

    let elf = guest.elf.to_vec();
    let image_id = guest.image_id;

    info!("Risc0 program compiled OK - {} bytes", elf.len());
    info!("Image ID - {image_id}");

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

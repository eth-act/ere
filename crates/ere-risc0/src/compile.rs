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

    // Use `risc0_build::build_package` to build package instead of calling
    // `cargo-risczero build` for the `unstable` features.
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
    use crate::RV32_IM_RISC0_ZKVM_ELF;
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::Compiler;

    #[test]
    fn test_compiler_impl() {
        let guest_directory = testing_guest_directory("risc0", "basic");
        let program = RV32_IM_RISC0_ZKVM_ELF.compile(&guest_directory).unwrap();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }
}

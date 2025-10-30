use crate::{compiler::Error, program::Risc0Program};
use ere_compile_utils::cargo_metadata;
use ere_zkvm_interface::compiler::Compiler;
use risc0_build::GuestOptions;
use std::path::Path;
use tracing::info;

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// Rust toolchain of Risc0.
pub struct RustRv32imaCustomized;

impl Compiler for RustRv32imaCustomized {
    type Error = Error;

    type Program = Risc0Program;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        info!("Compiling Risc0 program at {}", guest_directory.display());

        let metadata = cargo_metadata(guest_directory)?;
        let package = metadata.root_package().unwrap();

        // Use `risc0_build::build_package` to build package instead of calling
        // `cargo-risczero build` for the `unstable` features.
        let guest = risc0_build::build_package(
            package,
            &metadata.target_directory,
            GuestOptions::default(),
        )
        .map_err(|err| Error::BuildFailure {
            err,
            guest_path: guest_directory.to_path_buf(),
        })?
        .into_iter()
        .next()
        .ok_or(Error::Risc0BuildMissingGuest)?;

        let elf = guest.elf.to_vec();
        let image_id = guest.image_id;

        info!("Risc0 program compiled OK - {} bytes", elf.len());
        info!("Image ID - {image_id}");

        Ok(Risc0Program { elf, image_id })
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv32imaCustomized;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::compiler::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("risc0", "basic");
        let program = RustRv32imaCustomized.compile(&guest_directory).unwrap();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }
}

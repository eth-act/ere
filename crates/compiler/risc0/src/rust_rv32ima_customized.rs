use std::path::Path;

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::cargo_metadata;
use risc0_build::GuestOptions;
use tracing::info;

use crate::Error;

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// Rust toolchain of Risc0.
pub struct Risc0RustRv32imaCustomized;

impl Compiler for Risc0RustRv32imaCustomized {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let guest_directory = guest_directory.as_ref();
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

        info!("Risc0 program compiled OK - {} bytes", elf.len());

        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    use crate::Risc0RustRv32imaCustomized;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("risc0", "basic");
        let elf = Risc0RustRv32imaCustomized.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

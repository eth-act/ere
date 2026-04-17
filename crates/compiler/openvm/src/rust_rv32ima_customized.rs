use crate::Error;
use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CommonError, rustup_add_rust_src};
use openvm_build::{GuestOptions, get_rustup_toolchain_name};
use std::{fs, path::Path};

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// target `riscv32im-risc0-zkvm-elf`.
pub struct OpenVMRustRv32imaCustomized;

impl Compiler for OpenVMRustRv32imaCustomized {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        rustup_add_rust_src(&get_rustup_toolchain_name())?;

        // Inlining `openvm_sdk::Sdk::build` in order to get raw elf bytes.
        let guest_directory = guest_directory.as_ref();
        let pkg = openvm_build::get_package(guest_directory);
        let guest_opts = GuestOptions::default().with_profile("release".to_string());
        let target_dir = match openvm_build::build_guest_package(&pkg, &guest_opts, None, &None) {
            Ok(target_dir) => target_dir,
            Err(Some(code)) => return Err(Error::BuildFailed(code))?,
            Err(None) => return Err(Error::BuildSkipped)?,
        };

        let elf_path = openvm_build::find_unique_executable(guest_directory, target_dir, &None)
            .map_err(Error::UniqueElfNotFound)?;
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;

        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use crate::OpenVMRustRv32imaCustomized;
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("openvm", "basic");
        let elf = OpenVMRustRv32imaCustomized
            .compile(guest_directory)
            .unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

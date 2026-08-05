use std::{fs, path::Path};

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CommonError, parse_cargo_build_options};
use openvm_build::GuestOptions;

use crate::Error;

/// Compiler for Rust guest program to RV64IMA architecture, using customized
/// Rust toolchain of OpenVM and target `riscv64im-unknown-openvm-elf`.
pub struct OpenVMRustRv64imaCustomized;

impl Compiler for OpenVMRustRv64imaCustomized {
    type Error = Error;

    fn compile(
        &self,
        guest_directory: impl AsRef<Path>,
        args: &[String],
    ) -> Result<Elf, Self::Error> {
        let extra_rustflags = std::env::var("ERE_RUSTFLAGS").unwrap_or_default();

        // Inlining `openvm_sdk::Sdk::build` in order to get raw elf bytes.
        let guest_directory = guest_directory.as_ref();
        let pkg = openvm_build::get_package(guest_directory);
        let options = parse_cargo_build_options(args)?;
        let guest_opts = GuestOptions::default()
            .with_rustc_flags(extra_rustflags.split_whitespace().map(String::from))
            .with_profile("release".to_string())
            .with_features(options.features)
            .with_options(
                options
                    .ignore_rust_version
                    .then_some("--ignore-rust-version"),
            );
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
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    use crate::OpenVMRustRv64imaCustomized;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("openvm", "basic");
        let elf = OpenVMRustRv64imaCustomized
            .compile(guest_directory, &[])
            .unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

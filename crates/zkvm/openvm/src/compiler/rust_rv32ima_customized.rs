use crate::{compiler::OpenVMProgram, error::CompileError};
use ere_compile_utils::CommonError;
use ere_zkvm_interface::Compiler;
use openvm_build::GuestOptions;
use std::{fs, path::Path};

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// target `riscv32im-risc0-zkvm-elf`.
pub struct RustRv32imaCustomized;

impl Compiler for RustRv32imaCustomized {
    type Error = CompileError;

    type Program = OpenVMProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        // Inlining `openvm_sdk::Sdk::build` in order to get raw elf bytes.
        let pkg = openvm_build::get_package(guest_directory);
        let guest_opts = GuestOptions::default().with_profile("release".to_string());
        let target_dir = match openvm_build::build_guest_package(&pkg, &guest_opts, None, &None) {
            Ok(target_dir) => target_dir,
            Err(Some(code)) => return Err(CompileError::BuildFailed(code))?,
            Err(None) => return Err(CompileError::BuildSkipped)?,
        };

        let elf_path = openvm_build::find_unique_executable(guest_directory, target_dir, &None)
            .map_err(CompileError::UniqueElfNotFound)?;
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;

        OpenVMProgram::from_elf_and_app_config_path(elf, guest_directory.join("openvm.toml"))
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv32imaCustomized;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("openvm", "basic");
        let program = RustRv32imaCustomized.compile(&guest_directory).unwrap();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }
}

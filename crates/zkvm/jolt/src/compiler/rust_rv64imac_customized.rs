use crate::{compiler::Error, program::JoltProgram};
use ere_compile_utils::{CommonError, cargo_metadata};
use ere_zkvm_interface::compiler::Compiler;
use jolt_core::host::Program;
use std::{env::set_current_dir, fs, path::Path};
use tempfile::tempdir;

/// Compiler for Rust guest program to RV64IMAC architecture, using customized
/// Rust toolchain of Jolt.
pub struct RustRv64imacCustomized;

impl Compiler for RustRv64imacCustomized {
    type Error = Error;

    type Program = JoltProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let guest_directory = guest_directory
            .canonicalize()
            .map_err(|err| CommonError::canonicalize(guest_directory, err))?;

        // Change current directory for `Program::build` to build guest program.
        set_current_dir(&guest_directory).map_err(|err| Error::SetCurrentDirFailed {
            err,
            path: guest_directory.to_path_buf(),
        })?;

        let metadata = cargo_metadata(&guest_directory)?;
        let package_name = &metadata.root_package().unwrap().name;

        let tempdir = tempdir().map_err(CommonError::tempdir)?;

        // Note that if this fails, it will panic, hence we need to catch it.
        let elf_path = std::panic::catch_unwind(|| {
            let mut program = Program::new(package_name);
            program.set_std(true);
            program.build(&tempdir.path().to_string_lossy());
            program.elf.unwrap()
        })
        .map_err(|_| Error::BuildFailed)?;

        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;

        Ok(JoltProgram { elf })
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv64imacCustomized;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::compiler::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("jolt", "basic");
        let program = RustRv64imacCustomized.compile(&guest_directory).unwrap();
        assert!(!program.elf().is_empty(), "ELF bytes should not be empty.");
    }
}

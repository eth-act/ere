use crate::{compiler::JoltProgram, error::CompileError};
use ere_compile_utils::{CommonError, cargo_metadata};
use ere_zkvm_interface::Compiler;
use jolt::host::DEFAULT_TARGET_DIR;
use std::{env::set_current_dir, fs, path::Path};

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// Rust toolchain of Jolt.
pub struct RustRv32imaCustomized;

impl Compiler for RustRv32imaCustomized {
    type Error = CompileError;

    type Program = JoltProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        // Change current directory for `Program::build` to build guest program.
        set_current_dir(guest_directory).map_err(|err| CompileError::SetCurrentDirFailed {
            err,
            path: guest_directory.to_path_buf(),
        })?;

        let metadata = cargo_metadata(guest_directory)?;
        let package_name = &metadata.root_package().unwrap().name;

        // Note that if this fails, it will panic, hence we need to catch it.
        let elf_path = std::panic::catch_unwind(|| {
            let mut program = jolt::host::Program::new(package_name);
            program.set_std(true);
            program.build(DEFAULT_TARGET_DIR);
            program.elf.unwrap()
        })
        .map_err(|_| CompileError::BuildFailed)?;

        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;

        Ok(elf)
    }
}

#[cfg(test)]
mod tests {
    use crate::{EreJolt, compiler::RustRv32imaCustomized};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{Compiler, ProverResourceType, zkVM};

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("jolt", "basic");
        let elf = RustRv32imaCustomized.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("jolt", "basic");
        let program = RustRv32imaCustomized.compile(&guest_directory).unwrap();
        let zkvm = EreJolt::new(program, ProverResourceType::Cpu).unwrap();

        zkvm.execute(&[]).unwrap();
    }
}

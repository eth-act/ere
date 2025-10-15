use crate::{
    compiler::NexusProgram,
    error::{CompileError, NexusError},
};
use ere_compile_utils::cargo_metadata;
use ere_zkvm_interface::Compiler;
use nexus_sdk::compile::{Compile, Compiler as NexusCompiler, cargo::CargoPackager};
use std::{fs, path::Path};

/// Compiler for Rust guest program to RV32I architecture.
pub struct RustRv32i;

impl Compiler for RustRv32i {
    type Error = NexusError;

    type Program = NexusProgram;

    fn compile(&self, guest_path: &Path) -> Result<Self::Program, Self::Error> {
        // 1. Check guest path
        if !guest_path.exists() {
            return Err(CompileError::PathNotFound(guest_path.to_path_buf()))?;
        }
        std::env::set_current_dir(guest_path).map_err(|e| CompileError::Client(e.into()))?;

        let metadata = cargo_metadata(guest_path).map_err(CompileError::CompileUtilError)?;
        let package_name = &metadata.root_package().unwrap().name;

        let mut prover_compiler = NexusCompiler::<CargoPackager>::new(package_name);
        let elf_path = prover_compiler
            .build()
            .map_err(|e| CompileError::Client(e.into()))?;

        let elf = fs::read(&elf_path).map_err(|_| CompileError::ElfNotFound(elf_path))?;

        Ok(elf)
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv32i;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("nexus", "basic");
        let elf = RustRv32i.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}

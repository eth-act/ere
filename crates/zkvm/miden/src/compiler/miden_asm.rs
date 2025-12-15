use crate::{
    compiler::Error,
    program::{MidenProgram, MidenSerdeWrapper},
};
use ere_zkvm_interface::compiler::Compiler;
use miden_assembly::Assembler;
use miden_core_lib::CoreLibrary;
use std::{fs, path::Path};

/// Compiler for Miden assembly guest program.
pub struct MidenAsm;

impl Compiler for MidenAsm {
    type Error = Error;
    type Program = MidenProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let dir_name = guest_directory
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(Error::InvalidProgramPath)?;

        let entrypoint = format!("{dir_name}.masm");
        let entrypoint_path = guest_directory.join(&entrypoint);
        if !entrypoint_path.exists() {
            return Err(Error::MissingEntrypoint {
                program_dir: guest_directory.display().to_string(),
                entrypoint,
            });
        }
        let source = fs::read_to_string(&entrypoint_path).map_err(|err| Error::ReadEntrypoint {
            entrypoint_path,
            err,
        })?;

        // Compile using Miden assembler
        let mut assembler = Assembler::default();
        assembler
            .link_dynamic_library(CoreLibrary::default())
            .map_err(Error::LoadCoreLibrary)?;

        let program = assembler
            .assemble_program(&source)
            .map_err(Error::AssemblyCompilation)?;

        Ok(MidenSerdeWrapper(program))
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::MidenAsm;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::compiler::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("miden", "fib");
        let program = MidenAsm.compile(&guest_directory).unwrap();
        assert!(program.0.num_procedures() > 0);
    }
}

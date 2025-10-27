use crate::{compiler::MidenProgram, error::CompileError};
use ere_zkvm_interface::Compiler;
use miden_assembly::Assembler;
use miden_stdlib::StdLibrary;
use std::{env, fs, path::Path};

/// Compiler for Miden assembly guest program.
pub struct MidenAsm;

impl Compiler for MidenAsm {
    type Error = CompileError;
    type Program = MidenProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let dir_name = guest_directory
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(CompileError::InvalidProgramPath)?;

        let entrypoint = format!("{dir_name}.masm");
        let entrypoint_path = guest_directory.join(&entrypoint);
        if !entrypoint_path.exists() {
            return Err(CompileError::MissingEntrypoint {
                program_dir: guest_directory.display().to_string(),
                entrypoint,
            });
        }
        let source =
            fs::read_to_string(&entrypoint_path).map_err(|err| CompileError::ReadEntrypoint {
                entrypoint_path,
                err,
            })?;

        // Compile using Miden assembler
        let mut assembler =
            Assembler::default().with_debug_mode(env::var_os("MIDEN_DEBUG").is_some());
        assembler
            .link_dynamic_library(StdLibrary::default())
            .map_err(CompileError::LoadStdLibrary)?;

        let program = assembler
            .assemble_program(&source)
            .map_err(CompileError::AssemblyCompilation)?;

        Ok(MidenProgram(program))
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::MidenAsm;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("miden", "fib");
        let program = MidenAsm.compile(&guest_directory).unwrap();
        assert!(program.0.num_procedures() > 0);
    }
}

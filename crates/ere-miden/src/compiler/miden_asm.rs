use crate::{
    compiler::MidenProgram,
    error::{CompileError, MidenError},
};
use miden_assembly::Assembler;
use miden_stdlib::StdLibrary;
use std::{fs, path::Path};
use zkvm_interface::Compiler;

/// Compiler for Miden assembly guest program.
pub struct MidenAsm;

impl Compiler for MidenAsm {
    type Error = MidenError;
    type Program = MidenProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let dir_name = guest_directory
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(CompileError::InvalidProgramPath)?;

        let entrypoint = format!("{dir_name}.masm");
        let main_path = guest_directory.join(&entrypoint);
        if !main_path.exists() {
            return Err(CompileError::MissingEntrypoint {
                program_dir: guest_directory.display().to_string(),
                entrypoint,
            }
            .into());
        }

        // Compile using Miden assembler
        let mut assembler = Assembler::default().with_debug_mode(true);
        assembler
            .link_dynamic_library(StdLibrary::default())
            .map_err(|e| CompileError::LoadStdLibrary(e.to_string()))?;

        let source = fs::read_to_string(&main_path).map_err(|e| CompileError::ReadSource {
            path: main_path.clone(),
            source: e,
        })?;

        let program = assembler
            .assemble_program(&source)
            .map_err(|e| CompileError::AssemblyCompilation(e.to_string()))?;

        Ok(MidenProgram(program))
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::MidenAsm;
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("miden", "fib");
        let program = MidenAsm.compile(&guest_directory).unwrap();
        assert!(program.0.num_procedures() > 0);
    }
}

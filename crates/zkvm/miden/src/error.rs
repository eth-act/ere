use miden_assembly::Report;
use miden_processor::ExecutionError;
use miden_verifier::VerificationError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Invalid program directory name")]
    InvalidProgramPath,

    #[error("Entrypoint `{entrypoint}` not found in {program_dir}")]
    MissingEntrypoint {
        program_dir: String,
        entrypoint: String,
    },

    #[error("Failed to read assembly source at {entrypoint_path}: {err}")]
    ReadEntrypoint {
        entrypoint_path: PathBuf,
        #[source]
        err: std::io::Error,
    },

    #[error("Failed to load Miden standard library: {0}")]
    LoadStdLibrary(Report),

    #[error("Miden assembly compilation failed: {0}")]
    AssemblyCompilation(Report),
}

#[derive(Debug, Error)]
pub enum MidenError {
    #[error(transparent)]
    CommonError(#[from] ere_zkvm_interface::CommonError),

    // Execute
    #[error("Miden execution failed")]
    Execute(#[from] ExecutionError),

    // Prove
    #[error("Miden proving failed: {0}")]
    Prove(#[source] ExecutionError),

    // Verify
    #[error("Miden verification failed")]
    Verify(#[from] VerificationError),
}

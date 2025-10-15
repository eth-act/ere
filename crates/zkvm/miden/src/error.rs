use ere_zkvm_interface::zkVMError;
use miden_core::utils::DeserializationError;
use miden_processor::ExecutionError;
use miden_verifier::VerificationError;
use std::path::PathBuf;
use thiserror::Error;

impl From<MidenError> for zkVMError {
    fn from(value: MidenError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum MidenError {
    #[error(transparent)]
    Compile(#[from] CompileError),
    #[error(transparent)]
    Execute(#[from] ExecuteError),
    #[error(transparent)]
    Prove(#[from] ProveError),
    #[error(transparent)]
    Verify(#[from] VerifyError),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Invalid program directory name")]
    InvalidProgramPath,
    #[error("Entrypoint '{entrypoint}' not found in {program_dir}")]
    MissingEntrypoint {
        program_dir: String,
        entrypoint: String,
    },
    #[error("Failed to read assembly source at {path}")]
    ReadSource {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Miden assembly compilation failed: {0}")]
    AssemblyCompilation(String),
    #[error("Failed to load Miden standard library: {0}")]
    LoadStdLibrary(String),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("Miden execution failed")]
    Execution(#[from] ExecutionError),
    #[error("Invalid input format: {0}")]
    InvalidInput(String),
    #[error("Serialization failed")]
    Serialization(#[from] bincode::Error),
    #[error("Failed to deserialize Miden program")]
    ProgramDeserialization(#[from] DeserializationError),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("Miden proving failed")]
    Proving(#[from] ExecutionError),
    #[error("Invalid input format: {0}")]
    InvalidInput(String),
    #[error("Serialization failed")]
    Serialization(#[from] bincode::Error),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Miden verification failed")]
    Verification(#[from] VerificationError),
    #[error("Proof or associated data deserialization failed")]
    MidenDeserialization(#[from] DeserializationError),
    #[error("Proof bundle deserialization failed")]
    BundleDeserialization(#[from] bincode::Error),
}

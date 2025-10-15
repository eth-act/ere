use ark_serialize::SerializationError;
use ere_zkvm_interface::zkVMError;
use jolt::jolt_core::utils::errors::ProofVerifyError;
use std::{io, path::PathBuf};
use thiserror::Error;

impl From<JoltError> for zkVMError {
    fn from(value: JoltError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum JoltError {
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
    #[error("Failed to build guest")]
    BuildFailed,
    #[error("Failed to read elf at {path}: {source}")]
    ReadElfFailed { source: io::Error, path: PathBuf },
    #[error("Failed to set current directory to {path}: {source}")]
    SetCurrentDirFailed { source: io::Error, path: PathBuf },
    #[error(transparent)]
    CompileUtilError(#[from] ere_compile_utils::CompileError),
}

#[derive(Debug, Error)]
pub enum ExecuteError {}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("Serialization failed")]
    Serialization(#[from] SerializationError),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Deserialization failed")]
    Serialization(#[from] SerializationError),
    #[error("Failed to verify proof: {0}")]
    VerifyProofFailed(#[from] ProofVerifyError),
}

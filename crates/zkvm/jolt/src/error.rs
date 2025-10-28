use ere_compile_utils::CommonError;
use jolt_core::utils::errors::ProofVerifyError;
use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Failed to set current directory to {path}: {err}")]
    SetCurrentDirFailed {
        path: PathBuf,
        #[source]
        err: io::Error,
    },

    #[error("Failed to build guest")]
    BuildFailed,
}

#[derive(Debug, Error)]
pub enum JoltError {
    #[error(transparent)]
    CommonError(#[from] ere_zkvm_interface::CommonError),

    // Execute
    #[error("Execution panics")]
    ExecutionPanic,

    // Verify
    #[error("Failed to verify proof: {0}")]
    VerifyProofFailed(#[from] ProofVerifyError),
}

use ere_zkvm_interface::ProofKind;
use thiserror::Error;
use zkm_sdk::{ZKMProofKind, ZKMVerificationError};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    CommonError(#[from] ere_compile_utils::CommonError),
}

#[derive(Debug, Error)]
pub enum ZirenError {
    // Execute
    #[error("Ziren execution failed: {0}")]
    Execute(#[source] anyhow::Error),

    // Prove
    #[error("Ziren proving failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("Ziren proving panicked: {0}")]
    ProvePanic(String),

    // Verify
    #[error("Invalid proof kind, expected: {0:?}, got: {1:?}")]
    InvalidProofKind(ProofKind, ZKMProofKind),

    #[error("Ziren verification failed: {0}")]
    Verify(#[source] ZKMVerificationError),
}

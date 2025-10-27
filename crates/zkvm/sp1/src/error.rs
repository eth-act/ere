use ere_zkvm_interface::ProofKind;
use sp1_sdk::{SP1ProofMode, SP1VerificationError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    CommonError(#[from] ere_compile_utils::CommonError),
}

#[derive(Debug, Error)]
pub enum SP1Error {
    #[error(transparent)]
    CommonError(#[from] ere_zkvm_interface::CommonError),

    // Execute
    #[error("SP1 execution failed: {0}")]
    Execute(#[source] anyhow::Error),

    // Prove
    #[error("SP1 SDK proving failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("SP1 proving panicked: {0}")]
    Panic(String),

    // Verify
    #[error("Invalid proof kind, expected: {0:?}, got: {1:?}")]
    InvalidProofKind(ProofKind, SP1ProofMode),

    #[error("SP1 SDK verification failed: {0}")]
    Verify(#[source] SP1VerificationError),
}

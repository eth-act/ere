use ere_zkvm_interface::zkvm::CommonError;
use jolt_core::utils::errors::ProofVerifyError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Output is expected to have length prefix")]
    InvalidOutput,

    // Execute
    #[error("Execution panics")]
    ExecutionPanic,

    // Verify
    #[error("Failed to construct verifier: {0}")]
    VerifierInitFailed(#[from] ProofVerifyError),

    #[error("Verification failed: {0}")]
    VerifyFailed(anyhow::Error),
}

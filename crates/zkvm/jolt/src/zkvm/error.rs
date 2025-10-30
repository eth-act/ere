use ere_zkvm_interface::zkvm::CommonError;
use jolt_core::utils::errors::ProofVerifyError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Execute
    #[error("Execution panics")]
    ExecutionPanic,

    // Verify
    #[error("Failed to verify proof: {0}")]
    VerifyProofFailed(#[from] ProofVerifyError),
}

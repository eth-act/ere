use ere_zkvm_interface::zkvm::CommonError;
use miden_processor::ExecutionError;
use miden_verifier::VerificationError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

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

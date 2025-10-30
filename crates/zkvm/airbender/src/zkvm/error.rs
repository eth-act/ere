use crate::zkvm::sdk::VkHashChain;
use ere_zkvm_interface::zkvm::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Execution
    #[error("Failed to parse public value from stdout: {0}")]
    ParsePublicValue(String),

    #[error("Failed to parse cycles from stdout: {0}")]
    ParseCycles(String),

    // Verification
    #[error("Proof verification failed")]
    ProofVerificationFailed,

    #[error("Invalid final register count, expected 32 but got {0}")]
    InvalidRegisterCount(usize),

    #[error(
        "Unexpected verification key hash chain - preprocessed: {preprocessed:?}, proved: {proved:?}"
    )]
    UnexpectedVkHashChain {
        preprocessed: VkHashChain,
        proved: VkHashChain,
    },
}

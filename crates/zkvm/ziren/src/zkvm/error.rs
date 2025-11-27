use ere_zkvm_interface::zkvm::ProofKind;
use thiserror::Error;
use zkm_sdk::{ZKMProofKind, ZKMVerificationError};

#[derive(Debug, Error)]
pub enum Error {
    // Execute
    #[error("Ziren execution failed: {0}")]
    Execute(#[source] anyhow::Error),

    #[error("Deserialize proofs in Input failed: {0:?}")]
    DeserializeInputProofs(bincode::error::DecodeError),

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

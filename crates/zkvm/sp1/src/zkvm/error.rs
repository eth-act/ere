use ere_zkvm_interface::zkvm::{CommonError, ProofKind};
use sp1_sdk::{SP1ProofMode, SP1VerificationError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Prover RwLock posioned, panic not catched properly")]
    RwLockPosioned,

    #[error("Deserialize proofs in Input failed: {0:?}")]
    DeserializeInputProofs(bincode::error::DecodeError),

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

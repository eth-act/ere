use core::ops::RangeInclusive;
use ere_zkvm_interface::zkvm::{CommonError, ProofKind};
use risc0_zkp::verify::VerificationError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Deserialize proofs in Input failed: {0:?}")]
    DeserializeInputProofs(bincode::error::DecodeError),

    #[error("Unsupported power of 2 value {val} of {key}, expected in range {range:?}")]
    UnsupportedPo2Value {
        key: String,
        val: String,
        range: RangeInclusive<usize>,
    },

    // Execute
    #[error("Failed to build `ExecutorEnv`: {0}")]
    BuildExecutorEnv(anyhow::Error),

    #[error("Failed to execute: {0}")]
    Execute(anyhow::Error),

    // Prove
    #[error("Failed to initialize cuda prover: {0}")]
    InitializeCudaProver(anyhow::Error),

    #[error("Failed to prove: {0}")]
    Prove(anyhow::Error),

    // Verify
    #[error("Invalid proof kind, expected: {0:?}, got: {1}")]
    InvalidProofKind(ProofKind, String),

    #[error("Failed to verify: {0}")]
    Verify(VerificationError),
}

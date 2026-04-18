use alloc::string::String;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// VK byte slice was not the expected 32 bytes.
    #[error("Invalid VK length, expected: {expected}, got: {got}")]
    InvalidLength { expected: usize, got: usize },

    /// Inner receipt was not `Succinct`.
    #[error("Unexpected proof kind, expected: Succinct, got: {0}")]
    UnexpectedProofKind(String),

    /// Upstream `risc0-zkp` rejected the proof.
    #[error("Failed to verify: {0}")]
    Verify(risc0_zkp::verify::VerificationError),
}

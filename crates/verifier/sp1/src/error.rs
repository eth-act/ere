use sp1_verifier::{SP1ProofMode, compressed::CompressedError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// VK byte slice was not the expected 32 bytes.
    #[error("Invalid ProgramVk length, expected: {expected}, got: {got}")]
    InvalidProgramVkLength { expected: usize, got: usize },

    /// Proof was not in the expected `Compressed` form.
    #[error("Unexpected proof kind, expected: Compressed, got: {0:?}")]
    UnexpectedProofKind(SP1ProofMode),

    /// Upstream `sp1-sdk` rejected the proof.
    #[error("Failed to verify: {0}")]
    Verify(#[from] CompressedError),
}

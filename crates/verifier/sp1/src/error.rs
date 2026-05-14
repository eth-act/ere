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

    /// Program VK byte slice contains non-canonical field element.
    #[error("Non-canonical ProgramVk")]
    NonCanonicalProgramVk,

    /// Proof was not in the expected `Compressed` form.
    #[error("Unexpected proof kind, expected: Compressed, got: {0:?}")]
    UnexpectedProofKind(SP1ProofMode),

    /// `sp1-verifier` rejected the proof.
    #[error("Failed to verify: {0}")]
    Verify(#[from] CompressedError),
}

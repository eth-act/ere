use sp1_verifier::{SP1ProofMode, compressed::CompressedError};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
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

    /// The proof's `sp1_version` is not [`SP1_CIRCUIT_VERSION`](crate::SP1_CIRCUIT_VERSION).
    #[error("Unexpected SP1 version, expected: {}", crate::SP1_CIRCUIT_VERSION)]
    UnexpectedSp1Version,

    /// `sp1-verifier` rejected the proof.
    #[error("Failed to verify: {0}")]
    Verify(#[from] CompressedError),

    /// The proof's public values do not hold an exit code.
    #[error("Failed to extract exit code from proof")]
    ExitCodeExtractionFailed,

    /// The proof's public values hold a non-zero exit code.
    #[error("Execution failed with exit code: {0}")]
    ExecutionFailed(i32),
}

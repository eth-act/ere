use sp1_sdk::SP1ProofMode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// Proof was not in the expected `Compressed` form.
    #[error("Unexpected proof kind, expected: Compressed, got: {0:?}")]
    UnexpectedProofKind(SP1ProofMode),

    /// Upstream `sp1-sdk` rejected the proof.
    #[error("Failed to verify: {0}")]
    Verify(#[from] sp1_sdk::SP1VerificationError),
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof or program VK.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// `verify_proof_in_unified_layer` returned `Err`.
    #[error("Invalid proof")]
    InvalidProof,

    /// Hash chain inside the proof did not match the expected one.
    #[error("Unexpected hash chain, expected: {expected:?}, got: {got:?}")]
    UnexpectedHashChain { expected: [u32; 8], got: [u32; 8] },
}

use thiserror::Error;

use crate::ZiskProgramVk;

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

    /// Proof did not match the expected layout.
    #[error("Invalid proof format: {0}")]
    InvalidProofFormat(String),

    /// `verify_vadcop_final_proof` returned false.
    #[error("Invalid proof")]
    InvalidProof,

    /// ProgramVk inside the proof did not match the expected one.
    #[error("Unexpected ProgramVk, expected: {expected:?}, got: {got:?}")]
    UnexpectedProgramVk {
        expected: ZiskProgramVk,
        got: ZiskProgramVk,
    },
}

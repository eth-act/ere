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

    /// `verify_vadcop_final` returned false.
    #[error("Invalid proof")]
    InvalidProof,

    /// ProgramVk inside the proof did not match the expected one.
    #[error("Unexpected ProgramVk, expected: {expected:?}, got: {got:?}")]
    UnexpectedProgramVk {
        expected: ZiskProgramVk,
        got: ZiskProgramVk,
    },
}

use crate::ZiskProgramVk;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// VK byte slice was not the expected 32 bytes.
    #[error("Invalid VK length, expected: {expected}, got: {got}")]
    InvalidLength { expected: usize, got: usize },

    /// Upstream `verify_vadcop_final` returned false.
    #[error("Failed to verify")]
    VerifyFailed,

    /// Proof byte length was not a multiple of 8.
    #[error("Invalid proof size, {0} is not a multiple of 8")]
    InvalidProofSize(usize),

    /// Program VK inside the proof did not match the expected one.
    #[error("Unexpected program VK, expected: {expected:?}, got: {got:?}")]
    UnexpectedProgramVk {
        expected: ZiskProgramVk,
        got: ZiskProgramVk,
    },
}

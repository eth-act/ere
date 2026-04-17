use thiserror::Error;

use crate::AirbenderProgramVk;

#[derive(Debug, Error)]
pub enum Error {
    /// VK byte slice was not the expected 32 bytes.
    #[error("Invalid VK length, expected: {expected}, got: {got}")]
    InvalidLength { expected: usize, got: usize },

    /// Upstream `verify_recursion_log_23_layer` returned false.
    #[error("Failed to verify")]
    VerifyFailed,

    /// Proof contained an unexpected number of register values.
    #[error("Invalid final register count, expected: 32, got: {0}")]
    InvalidRegisterCount(usize),

    /// Program VK recovered from the proof did not match the expected one.
    #[error("Unexpected program VK, expected: {expected:?}, got: {got:?}")]
    UnexpectedProgramVk {
        expected: AirbenderProgramVk,
        got: AirbenderProgramVk,
    },
}

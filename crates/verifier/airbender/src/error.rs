use thiserror::Error;

use crate::AirbenderProgramVk;

#[derive(Debug, Error)]
pub enum Error {
    /// VK byte slice was not the expected 32 bytes.
    #[error("Invalid ProgramVk length, expected: {expected}, got: {got}")]
    InvalidProgramVkLength { expected: usize, got: usize },

    /// `verify_recursion_log_23_layer` returned false.
    #[error("Invalid proof")]
    InvalidProof,

    /// Proof contained an unexpected number of register values.
    #[error("Invalid final register count, expected: 32, got: {0}")]
    InvalidRegisterCount(usize),

    /// ProgramVk recovered from the proof did not match the expected one.
    #[error("Unexpected ProgramVk, expected: {expected:?}, got: {got:?}")]
    UnexpectedProgramVk {
        expected: AirbenderProgramVk,
        got: AirbenderProgramVk,
    },
}

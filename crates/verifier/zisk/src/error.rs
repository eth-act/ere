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

    /// Expected compressed VadcopFinalProof
    #[error("Invalid kind of VadcopFinalProof, expected compressed")]
    InvalidVadcopFinalProofKind,

    /// Public values of VadcopFinalProof was not the expected 68 words.
    #[error("Invalid public value length of VadcopFinalProof, expected: {expected}, got: {got}")]
    InvalidPublicValueLength { expected: usize, got: usize },

    /// User public values was not u32.
    #[error("Invalid word in user public values, expected u32")]
    InvalidPublicValue,

    /// `verify_vadcop_final_compressed_u64` returned false.
    #[error("Invalid proof")]
    InvalidProof,

    /// ProgramVk inside the proof did not match the expected one.
    #[error("Unexpected ProgramVk, expected: {expected:?}, got: {got:?}")]
    UnexpectedProgramVk {
        expected: ZiskProgramVk,
        got: ZiskProgramVk,
    },
}

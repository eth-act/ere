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

    /// Expected uncompressed VadcopFinalProof
    #[error("Invalid kind of VadcopFinalProof, expected uncompressed")]
    InvalidVadcopFinalProofKind,

    /// Public values of VadcopFinalProof was not the expected length.
    #[error("Invalid public value length of VadcopFinalProof, expected: {expected}, got: {got}")]
    InvalidPublicValueLength { expected: usize, got: usize },

    /// Expected leaf VadcopFinalProof
    #[error("Invalid is_vadcop_final_proof flag of VadcopFinalProof, got: {got}")]
    UnexpectedVadcopFinalFlag { got: u64 },

    /// User public values was not u32.
    #[error("Invalid word in user public values, expected u32")]
    InvalidPublicValue,

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

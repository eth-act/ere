use openvm_verify_stark_host::error::VerifyStarkError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a program VK.
    #[error("Failed to decode program vk: {0}")]
    DecodeProgramVk(#[from] bitcode::Error),

    /// Failed to decode a proof.
    #[error("Failed to decode proof: {0}")]
    DecodeProof(#[from] std::io::Error),

    /// Failed to verify a STARK proof.
    #[error("Verification failed: {0}")]
    Verify(#[from] VerifyStarkError),

    /// A field element could not be downcast to `u8`.
    #[error("Invalid public value")]
    InvalidPublicValue,

    /// Public value size is not as expected.
    #[error("Invalid public value size, expected {expected}, got {got}")]
    InvalidPublicValueSize { expected: usize, got: usize },
}

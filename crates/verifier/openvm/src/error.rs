use openvm_verify_stark_host::error::VerifyStarkError;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Program VK is not 32 bytes.
    #[error("Invalid ProgramVk length, expected: {expected}, got: {got}")]
    InvalidProgramVkLength { expected: usize, got: usize },

    /// Program VK has an element not less than `F::ORDER_U32`.
    #[error("Non-canonical ProgramVk")]
    NonCanonicalProgramVk,

    /// Proof byte slice was longer than the decode limit.
    #[error("Proof size exceeds decode limit, expected at most {limit}, got {got}")]
    DecodeLimitExceeded { limit: usize, got: usize },

    /// Number of AIRs that the proof states differs from the number of AIRs of the aggregation key.
    #[error("Invalid number of AIRs, expected {expected}, got {got}")]
    InvalidNumAirs { expected: usize, got: usize },

    /// Failed to decode a proof.
    #[error("Failed to decode proof: {0}")]
    DecodeProof(#[from] std::io::Error),

    /// Failed to verify a STARK proof.
    #[error("Verification failed: {0}")]
    Verify(#[from] VerifyStarkError),

    /// A field element could not be downcast to `u16`.
    #[error("Invalid public value")]
    InvalidPublicValue,

    /// Public value size is not as expected.
    #[error("Invalid public value size, expected {expected}, got {got}")]
    InvalidPublicValueSize { expected: usize, got: usize },
}

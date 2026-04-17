use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to encode the proof via `openvm_sdk::codec`.
    #[error("Failed to encode proof: {0}")]
    Encode(#[source] std::io::Error),

    /// Failed to decode the proof via `openvm_sdk::codec`.
    #[error("Failed to decode proof: {0}")]
    Decode(#[source] std::io::Error),

    /// VK byte slice was not the expected 64 bytes.
    #[error("Invalid VK length, expected: {expected}, got: {got}")]
    InvalidLength { expected: usize, got: usize },

    /// Upstream `openvm_sdk` rejected the proof.
    #[error("Failed to verify: {0}")]
    Verify(#[from] openvm_sdk::SdkError),

    /// A field element could not be downcast to `u8`.
    #[error("Invalid public value")]
    InvalidPublicValue,
}

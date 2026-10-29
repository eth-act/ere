use pico_verifier::VerifyError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to decode a VK bundle, its whitelist root is not the one shipped with Pico, or its
    /// memberships do not reach that root.
    #[error("Failed to decode program vk: {0}")]
    DecodeProgramVk(#[source] VerifyError),

    /// Failed to verify a proof.
    #[error("Verification failed: {0}")]
    Verify(#[source] VerifyError),
}

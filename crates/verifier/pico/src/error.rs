use pico_verifier::VerifyError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to decode a VK bundle.
    #[error("Failed to decode program vk: {0}")]
    DecodeProgramVk(#[source] VerifyError),

    /// Failed to encode a VK bundle without memberships.
    #[error("Failed to encode program vk: {0}")]
    EncodeProgramVk(#[source] VerifyError),

    /// The whitelist root of a VK bundle is not the one shipped with Pico, or its memberships do
    /// not reach that root.
    #[error("Untrusted program vk: {0}")]
    UntrustedProgramVk(#[source] VerifyError),

    /// Failed to verify a proof.
    #[error("Verification failed: {0}")]
    Verify(#[source] VerifyError),
}

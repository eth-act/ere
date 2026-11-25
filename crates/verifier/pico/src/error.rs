use pico_verifier::VerifyError;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Program VK is not `4 * VM_VK_LEN` bytes.
    #[error("Invalid ProgramVk length, expected: {expected}, got: {got}")]
    InvalidProgramVkLength { expected: usize, got: usize },

    /// Program VK has a limb not less than the KoalaBear modulus.
    #[error("Non-canonical ProgramVk")]
    NonCanonicalProgramVk,

    /// Failed to verify a proof.
    #[error("Verification failed: {0}")]
    Verify(#[source] VerifyError),
}

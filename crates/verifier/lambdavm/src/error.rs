use lambda_vm_executor::elf::ElfError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Program VK byte slice length did not match its length prefix.
    #[error("Invalid program vk length, expected {expected}, got {got}")]
    InvalidProgramVkLength { expected: usize, got: usize },

    /// Failed to load the ELF of a program VK.
    #[error("Failed to decode program vk: {0}")]
    DecodeProgramVk(#[from] ElfError),

    /// Proof byte slice was longer than the decode limit.
    #[error("Proof size exceeds decode limit, expected at most {limit}, got {got}")]
    DecodeLimitExceeded { limit: usize, got: usize },

    /// Failed to decode a proof.
    #[error("Failed to decode proof: {0}")]
    DecodeProof(#[from] rkyv::rancor::Error),

    /// `BLOWUP_FACTOR` did not give valid proof options.
    #[error("Invalid proof options: {0}")]
    InvalidProofOptions(String),

    /// Failed to verify a STARK proof.
    #[error("Verification failed: {0}")]
    Verify(#[from] lambda_vm_prover::Error),

    /// `verify_with_options` returned false.
    #[error("Invalid proof")]
    InvalidProof,
}

use lambda_vm_executor::elf::ElfError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] rkyv::rancor::Error),

    /// Proof byte slice was longer than the decode limit.
    #[error("Decode limit exceeded, limit: {limit}, got: {got}")]
    DecodeLimitExceeded { limit: usize, got: usize },

    /// ProgramVk byte slice length did not match its length prefix.
    #[error("Invalid ProgramVk length, expected: {expected}, got: {got}")]
    InvalidProgramVkLength { expected: usize, got: usize },

    /// ProgramVk was not a loadable ELF.
    #[error("Invalid ProgramVk ELF: {0}")]
    InvalidProgramVkElf(#[from] ElfError),

    /// `BLOWUP_FACTOR` did not give valid proof options.
    #[error("Invalid proof options: {0}")]
    ProofOptions(String),

    /// `verify_with_options` returned false.
    #[error("Invalid proof")]
    InvalidProof,

    /// `lambda-vm-prover` rejected the proof.
    #[error("Failed to verify: {0}")]
    Verify(#[from] lambda_vm_prover::Error),
}

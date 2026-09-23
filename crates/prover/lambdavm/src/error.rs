use ere_prover_core::CommonError;
use lambda_vm_executor::{elf::ElfError, vm::execution::ExecutorError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
    #[error("Decode elf failed: {0}")]
    DecodeElf(ElfError),

    #[error("Invalid proof options: {0}")]
    InvalidProofOptions(String),

    // Execute
    #[error("LambdaVM execution failed: {0}")]
    Execute(#[source] ExecutorError),

    #[error("LambdaVM execution exceeded {0} cycles")]
    CycleLimitExceeded(u64),

    #[error("LambdaVM cost estimation failed: {0}")]
    EstimateCost(#[source] lambda_vm_prover::Error),

    // Prove
    #[error("LambdaVM proving failed: {0}")]
    Prove(#[source] lambda_vm_prover::Error),

    #[error(transparent)]
    Verifier(#[from] ere_verifier_lambdavm::Error),
}

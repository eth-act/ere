use ere_zkvm_interface::zkvm::CommonError;
use nexus_vm::error::VMError;
use nexus_vm_prover::{ProvingError, VerificationError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Parse ELF failed: {0}")]
    ParseElf(#[source] VMError),

    // Execute
    #[error("Nexus execution failed: {0}")]
    Execute(#[source] VMError),

    #[error("Guest panicked with exit code {0}")]
    GuestPanic(u32),

    // Prove
    #[error("Nexus proving failed: {0}")]
    Prove(#[source] ProvingError),

    // Verify
    #[error("Nexus verification failed: {0}")]
    Verify(#[source] VerificationError),
}

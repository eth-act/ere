use ere_zkvm_interface::zkvm::CommonError;
use nexus_sdk::stwo::seq::Error as StwoError;
use nexus_vm::error::VMError;
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

    // Prove
    #[error("Nexus proving failed: {0}")]
    Prove(#[source] StwoError),

    // Verify
    #[error("Nexus verification failed: {0}")]
    Verify(#[source] StwoError),
}

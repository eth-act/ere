use nexus_sdk::stwo::seq::Error as StwoError;
use nexus_vm::error::VMError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    CommonError(#[from] ere_compile_utils::CommonError),
}

#[derive(Debug, Error)]
pub enum NexusError {
    #[error(transparent)]
    CommonError(#[from] ere_zkvm_interface::CommonError),

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

use openvm_sdk::{SdkError, commit::AppExecutionCommit};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    CommonError(#[from] ere_compile_utils::CommonError),

    #[error("Failed to build guest, code: {0}")]
    BuildFailed(i32),

    #[error("Guest building skipped (OPENVM_SKIP_BUILD is set)")]
    BuildSkipped,

    #[error("Missing to find unique elf: {0}")]
    UniqueElfNotFound(eyre::Error),
}

#[derive(Debug, Error)]
pub enum OpenVMError {
    #[error(transparent)]
    CommonError(#[from] ere_zkvm_interface::CommonError),

    // Common
    #[error("Initialize SDK failed: {0}")]
    SdkInit(SdkError),

    #[error("Decode elf failed: {0}")]
    ElfDecode(eyre::Error),

    #[error("Transpile elf failed: {0}")]
    Transpile(SdkError),

    #[error("Read aggregation key failed: {0}")]
    ReadAggKeyFailed(eyre::Error),

    #[error("Initialize prover failed: {0}")]
    ProverInit(SdkError),

    #[error("Invalid public value")]
    InvalidPublicValue,

    // Execute
    #[error("OpenVM execution failed: {0}")]
    Execute(#[source] SdkError),

    // Prove
    #[error("OpenVM proving failed: {0}")]
    Prove(#[source] SdkError),

    #[error("Unexpected app commit: {proved:?}, expected: {preprocessed:?}")]
    UnexpectedAppCommit {
        preprocessed: Box<AppExecutionCommit>,
        proved: Box<AppExecutionCommit>,
    },

    // Verify
    #[error("OpenVM verification failed: {0}")]
    Verify(#[source] SdkError),
}

use ere_zkvm_interface::zkvm::CommonError;
use openvm_sdk::{SdkError, commit::AppExecutionCommit};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
    #[error("Invalid AppConfig: {0}")]
    InvalidAppConfig(toml::de::Error),

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

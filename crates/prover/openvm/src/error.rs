use ere_prover_core::CommonError;
use openvm_sdk::{SdkError, commit::AppExecutionCommit};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
    #[error("Enable `cuda` feature to enable `ProverResource::Gpu`")]
    CudaFeatureDisabled,

    #[error("Transpile elf failed: {0}")]
    Transpile(SdkError),

    #[error("Read aggregation key failed: {0}")]
    ReadAggKeyFailed(eyre::Error),

    #[error("Initialize prover failed: {0}")]
    ProverInit(SdkError),

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

    #[error(transparent)]
    Verifier(#[from] ere_verifier_openvm::Error),
}

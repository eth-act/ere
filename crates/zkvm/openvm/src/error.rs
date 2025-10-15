use openvm_sdk::{SdkError, commit::AppExecutionCommit};
use std::{io, path::PathBuf};
use thiserror::Error;
use zkvm_interface::zkVMError;

impl From<OpenVMError> for zkVMError {
    fn from(value: OpenVMError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

impl From<CommonError> for zkVMError {
    fn from(value: CommonError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum OpenVMError {
    #[error(transparent)]
    Compile(#[from] CompileError),

    #[error(transparent)]
    Execute(#[from] ExecuteError),

    #[error(transparent)]
    Prove(#[from] ProveError),

    #[error(transparent)]
    Verify(#[from] VerifyError),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Failed to build guest, code: {0}")]
    BuildFailed(i32),
    #[error("Guest building skipped (OPENVM_SKIP_BUILD is set)")]
    BuildSkipped,
    #[error("Missing to find unique elf: {0}")]
    UniqueElfNotFound(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Failed to read elf at {path}: {source}")]
    ReadElfFailed { source: io::Error, path: PathBuf },
    #[error("Failed to read OpenVM's config file at {path}: {source}")]
    ReadConfigFailed { source: io::Error, path: PathBuf },
    #[error("Failed to deserialize OpenVM's config file: {0}")]
    DeserializeConfigFailed(toml::de::Error),
    #[error(transparent)]
    CompileUtilError(#[from] compile_utils::CompileError),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("OpenVM execute failed: {0}")]
    Execute(#[source] SdkError),
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("OpenVM prove failed: {0}")]
    Prove(#[source] SdkError),
    #[error("Unexpected app commit: {proved:?}, expected: {preprocessed:?}")]
    UnexpectedAppCommit {
        preprocessed: AppExecutionCommit,
        proved: AppExecutionCommit,
    },
    #[error("Serialize proof failed: {0}")]
    SerializeProof(io::Error),
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("OpenVM verification failed: {0}")]
    Verify(#[source] SdkError),
    #[error("Deserialize proof failed: {0}")]
    DeserializeProof(io::Error),
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub enum CommonError {
    #[error("Initialize SDK failed: {0}")]
    SdkInit(SdkError),
    #[error("Decode elf failed: {0}")]
    ElfDecode(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Transpile elf failed: {0}")]
    Transpile(SdkError),
    #[error("Read aggregation key failed: {0}")]
    ReadAggKeyFailed(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Initialize prover failed: {0}")]
    ProverInit(SdkError),
    #[error("Invalid public value")]
    InvalidPublicValue,
}

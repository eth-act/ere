use std::{io, path::PathBuf};
use thiserror::Error;
use zkvm_interface::zkVMError;

impl From<OpenVMError> for zkVMError {
    fn from(value: OpenVMError) -> Self {
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
    DeserializeConfigFailed(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Failed to decode elf: {0}")]
    DecodeFailed(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Failed to transpile elf: {0}")]
    TranspileFailed(Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("OpenVM execute failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("OpenVM prove failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("OpenVM verification failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

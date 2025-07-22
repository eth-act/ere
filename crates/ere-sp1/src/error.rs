use std::path::PathBuf;

use thiserror::Error;
use zkvm_interface::zkVMError;

impl From<SP1Error> for zkVMError {
    fn from(value: SP1Error) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum SP1Error {
    #[error(transparent)]
    CompileError(#[from] CompileError),

    #[error(transparent)]
    Execute(#[from] ExecuteError),

    #[error(transparent)]
    Prove(#[from] ProveError),

    #[error(transparent)]
    Verify(#[from] VerifyError),
}

/// Errors that can be encountered while compiling a SP1 program
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Failed to build Docker image: {0}")]
    DockerImageBuildFailed(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Docker command failed to execute: {0}")]
    DockerCommandFailed(#[source] std::io::Error),
    #[error("Docker container run failed with status: {0}")]
    DockerContainerRunFailed(std::process::ExitStatus),
    #[error("Invalid mount path: {0}")]
    InvalidMountPath(PathBuf),
    #[error("Invalid guest program path: {0}")]
    InvalidGuestPath(PathBuf),
    #[error("Failed to create temporary directory: {0}")]
    CreatingTempOutputDirectoryFailed(#[source] std::io::Error),
    #[error("Failed to create temporary output path: {0}")]
    InvalidTempOutputPath(PathBuf),
    #[error("Failed to read compiled ELF program: {0}")]
    ReadCompiledELFProgram(#[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("SP1 execution failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("SP1 SDK proving failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Serialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::Error),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Deserialising proof failed: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("SP1 SDK verification failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

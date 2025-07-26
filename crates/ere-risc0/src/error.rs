use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Risc0Error {
    #[error(transparent)]
    Compile(#[from] CompileError),
}

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
    #[error("Failed to read image id: {0}")]
    ReadImageId(#[source] std::io::Error),
    #[error("Failed to compute image id: {0}")]
    ComputeImaegIdFailed(#[source] anyhow::Error),
}

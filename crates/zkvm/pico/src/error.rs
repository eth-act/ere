use ere_zkvm_interface::zkVMError;
use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;

impl From<PicoError> for zkVMError {
    fn from(value: PicoError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum PicoError {
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
    #[error("Failed to create temporary directory: {0}")]
    Tempdir(io::Error),
    /// Guest program directory does not exist.
    #[error("guest program directory not found: {0}")]
    PathNotFound(PathBuf),
    /// Failed to spawn or run `cargo pico build`.
    #[error("failed to run `cargo pico build`: {0}")]
    CargoPicoBuild(#[from] io::Error),
    /// `cargo pico build` exited with a non-zero status.
    #[error("`cargo pico build` failed with status {status:?}")]
    CargoPicoBuildFailed { status: ExitStatus },
    /// Expected ELF file was not produced.
    #[error("ELF file not found at {0}")]
    ElfNotFound(PathBuf),
    /// Reading the ELF file failed.
    #[error("failed to read ELF file at {path}: {source}")]
    ReadElf {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    CompileUtilError(#[from] ere_compile_utils::CompileError),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("Pico execution failed: {0}")]
    Client(anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("Pico proving failed: {0}")]
    Client(anyhow::Error),
    #[error("Serialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::error::EncodeError),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Pico verifying failed: {0}")]
    Client(anyhow::Error),
    #[error("Deserialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::error::DecodeError),
    #[error("Invalid base proof length {0}, expected 1")]
    InvalidBaseProofLength(usize),
    #[error("Invalid public values length {0}, expected at least 32")]
    InvalidPublicValuesLength(usize),
    #[error("First 32 public values are expected in byte")]
    InvalidPublicValues,
    #[error("Public values digest are expected in bytes")]
    InvalidPublicValuesDigest,
}

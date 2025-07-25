use std::{path::PathBuf, process::ExitStatus};

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
    #[error("Program path does not exist or is not a directory: {0}")]
    InvalidProgramPath(PathBuf),
    #[error(
        "Cargo.toml not found in program directory: {program_dir}. Expected at: {manifest_path}"
    )]
    CargoTomlMissing {
        program_dir: PathBuf,
        manifest_path: PathBuf,
    },
    #[error("Could not find `[package].name` in guest Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf },
    #[error("Compiled ELF not found at expected path: {0}")]
    ElfNotFound(PathBuf),
    #[error("`cargo prove build` failed with status: {status} for program at {path}")]
    CargoBuildFailed { status: ExitStatus, path: PathBuf },
    #[error("Failed to read file at {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse guest Cargo.toml at {path}: {source}")]
    ParseCargoToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("Failed to execute `cargo prove build` in {cwd}: {source}")]
    CargoProveBuild {
        cwd: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to create temporary output directory: {0}")]
    TempDir(#[from] std::io::Error),
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

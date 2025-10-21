use ere_zkvm_interface::{ProofKind, zkVMError};
use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;
use zkm_sdk::ZKMProofKind;

impl From<ZirenError> for zkVMError {
    fn from(value: ZirenError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum ZirenError {
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
    #[error("`RUSTUP_TOOLCHAIN=zkm rustc --print sysroot` failed to execute: {0}")]
    RustcSysrootFailed(#[source] io::Error),
    #[error(
        "`RUSTUP_TOOLCHAIN=zkm rustc --print sysroot` exited with non-zero status {status}, stdout: {stdout}, stderr: {stderr}"
    )]
    RustcSysrootExitNonZero {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error("`cargo ziren build` failed to execute: {0}")]
    CargoZirenBuildFailed(#[source] io::Error),
    #[error(
        "`cargo ziren build` exited with non-zero status {status}, stdout: {stdout}, stderr: {stderr}"
    )]
    CargoZirenBuildExitNonZero {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error("Failed to find guest in built packages")]
    GuestNotFound { name: String },
    #[error("Failed to read file at {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    CompileUtilError(#[from] ere_compile_utils::CompileError),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("Ziren execution failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("Serialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::error::EncodeError),

    #[error("Ziren proving failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Ziren proving panicked: {0}")]
    Panic(String),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Deserialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::error::DecodeError),

    #[error("Invalid proof kind, expected: {}, got: {}", 0.to_string(), 1.to_string() )]
    InvalidProofKind(ProofKind, ZKMProofKind),

    #[error("Ziren verification failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;
use zkm_sdk::ZKMProofKind;
use zkvm_interface::zkVMError;

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
    CompileUtilError(#[from] compile_utils::CompileError),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("Ziren execution failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("Serialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Ziren proving failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Deserialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Expect to get Compressed proof, but got: {}", 0.to_string())]
    InvalidProofKind(ZKMProofKind),

    #[error("Ziren verification failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

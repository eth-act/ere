use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Risc0Error {
    #[error(transparent)]
    Compile(#[from] CompileError),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("{context}: {source}")]
    Io {
        #[source]
        source: io::Error,
        context: &'static str,
    },
    #[error("Guest crate path does not exist or is not a directory: {0}")]
    InvalidGuestPath(PathBuf),
    #[error(
        "`cargo risczero build` for {crate_path} failed with status {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    CargoRisczeroBuildFailure {
        crate_path: PathBuf,
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error("Could not find image id and elf path in `cargo risczero build` output")]
    MissingImageIdAndElfPath,
    #[error("Invalid image id {0}")]
    InvalidImageId(String),
    #[error("Could not elf at {0}")]
    InvalidElfPath(PathBuf),
}

impl CompileError {
    pub fn io(e: io::Error, context: &'static str) -> Self {
        Self::Io { source: e, context }
    }
}

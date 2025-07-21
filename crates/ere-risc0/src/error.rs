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
    #[error("{context}: {source}")]
    SerdeJson {
        #[source]
        source: serde_json::Error,
        context: &'static str,
    },
    #[error("Methods crate path does not exist or is not a directory: {0}")]
    InvalidMethodsPath(PathBuf),
    #[error(
        "`cargo build` for {crate_path} failed with status {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    CargoBuildFailure {
        crate_path: PathBuf,
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error("Could not find field `{field}` in JSON file `{file}`")]
    MissingJsonField { field: &'static str, file: PathBuf },
}

impl CompileError {
    pub fn io(e: io::Error, context: &'static str) -> Self {
        Self::Io { source: e, context }
    }
    pub fn serde(e: serde_json::Error, context: &'static str) -> Self {
        Self::SerdeJson { source: e, context }
    }
}

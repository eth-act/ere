use ere_compile_utils::CommonError;
use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Failed to set current directory to {path}: {err}")]
    SetCurrentDirFailed {
        path: PathBuf,
        #[source]
        err: io::Error,
    },

    #[error("Failed to build guest")]
    BuildFailed,
}

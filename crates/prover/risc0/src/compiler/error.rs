use ere_util_compile::CommonError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("`risc0_build::build_package` for {guest_path} failed: {err}")]
    BuildFailure {
        #[source]
        err: anyhow::Error,
        guest_path: PathBuf,
    },

    #[error("`risc0_build::build_package` succeeded but failed to find guest")]
    Risc0BuildMissingGuest,
}

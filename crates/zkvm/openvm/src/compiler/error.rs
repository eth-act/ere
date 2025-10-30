use ere_compile_utils::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Failed to build guest, code: {0}")]
    BuildFailed(i32),

    #[error("Guest building skipped (OPENVM_SKIP_BUILD is set)")]
    BuildSkipped,

    #[error("Missing to find unique elf: {0}")]
    UniqueElfNotFound(eyre::Error),
}

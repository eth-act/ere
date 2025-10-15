use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Risc0Error {
    #[error(transparent)]
    Compile(#[from] CompileError),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("`risc0_build::build_package` for {crate_path} failed: {source}")]
    BuildFailure {
        #[source]
        source: anyhow::Error,
        crate_path: PathBuf,
    },
    #[error("`risc0_build::build_package` succeeded but failed to find guest")]
    Risc0BuildMissingGuest,
    #[error("ELF binary image calculation failure : {0}")]
    ImageIDCalculationFailure(anyhow::Error),
    #[error(transparent)]
    CompileUtilError(#[from] ere_compile_utils::CompileError),
}

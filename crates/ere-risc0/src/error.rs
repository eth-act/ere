use std::{io, path::PathBuf};
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
    #[error("`cargo metadata` failed: {0}")]
    MetadataCommand(#[from] cargo_metadata::Error),
    #[error("Could not find `[package].name` in guest Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf },
    #[error("`risc0_build::build_package` for {crate_path} failed: {source}")]
    BuildFailure {
        #[source]
        source: anyhow::Error,
        crate_path: PathBuf,
    },
    #[error("`risc0_build::build_package` succeeded but failed to find guest")]
    Risc0BuildMissingGuest,
    #[error("Failed to read file at {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("ELF binary image calculation failure : {0}")]
    ImageIDCalculationFailure(#[from] anyhow::Error),
}

impl CompileError {
    pub fn io(e: io::Error, context: &'static str) -> Self {
        Self::Io { source: e, context }
    }
}

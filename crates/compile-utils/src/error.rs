use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("`cargo metadata` in {manifest_dir} failed: {err}")]
    CargoMetadata {
        manifest_dir: PathBuf,
        err: cargo_metadata::Error,
    },
    #[error("Root package not found in {0}")]
    RootPackageNotFound(PathBuf),
    #[error("Failed to create temporary directory: {0}")]
    Tempdir(io::Error),
    #[error("Failed to create linker script: {0}")]
    CreateLinkerScript(io::Error),
    #[error("Failed to run `cargo build`: {0}")]
    CargoBuild(io::Error),
    #[error("`cargo build` failed: {0}")]
    CargoBuildFailed(ExitStatus),
    #[error("Failed to read built ELF: {0}")]
    ReadElf(io::Error),
}

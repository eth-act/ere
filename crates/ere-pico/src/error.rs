use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;
use zkvm_interface::zkVMError;

impl From<PicoError> for zkVMError {
    fn from(value: PicoError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum PicoError {
    /// Guest program directory does not exist.
    #[error("guest program directory not found: {0}")]
    PathNotFound(PathBuf),

    /// Failed to spawn or run `cargo pico build`.
    #[error("failed to run `cargo pico build`: {0}")]
    Spawn(#[from] io::Error),

    /// `cargo pico build` exited with a non-zero status.
    #[error("`cargo pico build` failed with status {status:?}")]
    CargoFailed { status: ExitStatus },

    /// Expected ELF file was not produced.
    #[error("ELF file not found at {0}")]
    ElfNotFound(PathBuf),

    /// Reading the ELF file failed.
    #[error("failed to read ELF file at {path}: {source}")]
    ReadElf {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Pico build failure for {crate_path} failed: {source}")]
    BuildFailure {
        #[source]
        source: anyhow::Error,
        crate_path: PathBuf,
    },
    #[error("Could not find `[package].name` in guest Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf },
    #[error("Failed to read file at {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`cargo metadata` failed: {0}")]
    MetadataCommand(#[from] cargo_metadata::Error),
}

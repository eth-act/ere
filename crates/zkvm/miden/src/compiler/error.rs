use miden_assembly::Report;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid program directory name")]
    InvalidProgramPath,

    #[error("Entrypoint `{entrypoint}` not found in {program_dir}")]
    MissingEntrypoint {
        program_dir: String,
        entrypoint: String,
    },

    #[error("Failed to read assembly source at {entrypoint_path}: {err}")]
    ReadEntrypoint {
        entrypoint_path: PathBuf,
        #[source]
        err: std::io::Error,
    },

    #[error("Failed to load Miden core library: {0}")]
    LoadCoreLibrary(Report),

    #[error("Miden assembly compilation failed: {0}")]
    AssemblyCompilation(Report),
}

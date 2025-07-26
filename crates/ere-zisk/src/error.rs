use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;
use zkvm_interface::zkVMError;

impl From<ZiskError> for zkVMError {
    fn from(value: ZiskError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum ZiskError {
    #[error(transparent)]
    Compile(#[from] CompileError),

    #[error(transparent)]
    Execute(#[from] ExecuteError),

    #[error(transparent)]
    Prove(#[from] ProveError),

    #[error(transparent)]
    Verify(#[from] VerifyError),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Failed to create temporary output directory: {0}")]
    TempDir(#[from] std::io::Error),
    #[error("Program path does not exist or is not a directory: {0}")]
    InvalidProgramPath(PathBuf),
    #[error(
        "Cargo.toml not found in program directory: {program_dir}. Expected at: {manifest_path}"
    )]
    CargoTomlMissing {
        program_dir: PathBuf,
        manifest_path: PathBuf,
    },
    #[error("Could not find `[package].name` in guest Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf },
    #[error("Compiled ELF not found at expected path: {path}")]
    ElfNotFound {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Failed to read file at {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Failed to parse guest Cargo.toml at {path}: {source}")]
    ParseCargoToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("Failed to execute `RUSTUP_TOOLCHAIN=zisk rustc --print sysroot`")]
    RustcSysroot {
        #[source]
        source: io::Error,
    },
    #[error("Failed to execute `cargo locate-project --workspace --message-format=plain`")]
    CargoLocateProject {
        #[source]
        source: io::Error,
    },
    #[error("Failed to execute `RUSTC=$ZISK_RUSTC cargo build --release ...` in {cwd}: {source}")]
    CargoBuild {
        cwd: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "`RUSTC=$ZISK_RUSTC cargo build --release ...` failed with status: {status} for program at {path}"
    )]
    CargoBuildFailed { status: ExitStatus, path: PathBuf },
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("IO failure in temporary directory: {0}")]
    TempDir(io::Error),
    #[error("Failed to serialize input: {0}")]
    SerializeInput(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to execute `ziskemu`: {source}")]
    Ziskemu {
        #[source]
        source: io::Error,
    },
    #[error("`ziskemu` failed with status: {status}")]
    ZiskemuFailed { status: ExitStatus },
    #[error("Total steps not found in report")]
    TotalStepsNotFound,
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("IO failure in temporary directory: {0}")]
    TempDir(io::Error),
    #[error("Failed to serialize input: {0}")]
    SerializeInput(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to execute `cargo-zisk rom-setup`: {source}")]
    CargoZiskRomSetup {
        #[source]
        source: io::Error,
    },
    #[error("`cargo-zisk rom-setup` failed with status: {status}")]
    CargoZiskRomSetupFailed { status: ExitStatus },
    #[error("Failed to execute `cargo prove`: {source}")]
    CargoZiskProve {
        #[source]
        source: io::Error,
    },
    #[error("`cargo prove` failed with status: {status}")]
    CargoZiskProveFailed { status: ExitStatus },
    #[error("Serialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::Error),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("IO failure in temporary directory: {0}")]
    TempDir(io::Error),
    #[error("Deserialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("Failed to execute `cargo-zisk verify`: {source}")]
    CargoZiskVerify {
        #[source]
        source: io::Error,
    },
    #[error("Invalid proof: {0}")]
    InvalidProof(String),
}

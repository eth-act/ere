use crate::client::RomDigest;
use bytemuck::PodCastError;
use ere_zkvm_interface::zkVMError;
use std::{io, path::PathBuf, process::ExitStatus};
use thiserror::Error;

impl From<ZiskError> for zkVMError {
    fn from(value: ZiskError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum ZiskError {
    // IO and file system
    #[error("IO failure: {0}")]
    Io(#[from] io::Error),
    #[error("IO failure in temporary directory: {0}")]
    TempDir(io::Error),
    #[error("Failed to read file at {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Failed to write file at {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    // Compilation
    #[error("Failed to execute `RUSTUP_TOOLCHAIN=zisk rustc --print sysroot`")]
    RustcSysroot(#[source] io::Error),
    #[error("Failed to execute `cargo locate-project --workspace --message-format=plain`")]
    CargoLocateProject(#[source] io::Error),
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
    #[error(transparent)]
    CompileUtilError(#[from] ere_compile_utils::CompileError),

    // Serialization
    #[error("Bincode encode failed: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    #[error("Bincode decode failed: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),

    // Execution
    #[error("Failed to execute `ziskemu`: {0}")]
    Ziskemu(#[source] io::Error),
    #[error("`ziskemu` failed with status: {status}")]
    ZiskemuFailed { status: ExitStatus },
    #[error("Total steps not found in execution report")]
    TotalStepsNotFound,

    // Check setup
    #[error("Failed to execute `cargo-zisk check-setup`: {0}")]
    CargoZiskCheckSetup(#[source] io::Error),
    #[error("`cargo-zisk check-setup` failed with status: {status}")]
    CargoZiskCheckSetupFailed { status: ExitStatus },

    // Rom setup
    #[error("Failed to execute `cargo-zisk rom-setup`: {0}")]
    CargoZiskRomSetup(#[source] io::Error),
    #[error("`cargo-zisk rom-setup` failed with status: {status}")]
    CargoZiskRomSetupFailed { status: ExitStatus },
    #[error("Failed to find ROM digest in output")]
    RomDigestNotFound,
    #[error("`cargo-zisk rom-setup` failed in another thread")]
    RomSetupFailedBefore,

    // Prove
    #[error("Mutex of ZiskServer is poisoned")]
    MutexPoisoned,
    #[error("Failed to execute `cargo-zisk server`: {0}")]
    CargoZiskServer(#[source] io::Error),
    #[error("Timeout waiting for server ready")]
    TimeoutWaitingServerReady,
    #[error("Failed to execute `cargo-zisk prove-client status`: {0}")]
    CargoZiskStatus(#[source] io::Error),
    #[error("`cargo-zisk prove-client status` failed with status: {status}")]
    CargoZiskStatusFailed { status: ExitStatus },
    #[error("Uknown server status")]
    UnknownServerStatus,
    #[error("Failed to execute `cargo-zisk prove-client prove`: {0}")]
    CargoZiskProve(#[source] io::Error),
    #[error("`cargo-zisk prove-client prove` failed with status: {status}")]
    CargoZiskProveFailed { status: ExitStatus },

    // Verify
    #[error("Failed to execute `cargo-zisk verify`: {0}")]
    CargoZiskVerify(#[source] io::Error),
    #[error("Invalid proof: {0}")]
    InvalidProof(String),
    #[error("Cast proof to `u64` slice failed: {0}")]
    CastProofBytesToU64s(PodCastError),
    #[error("Invalid public value format")]
    InvalidPublicValue,
    #[error("Public values length {0}, but expected at least 6")]
    InvalidPublicValuesLength(usize),
    #[error("Unexpected ROM digest - preprocessed: {preprocessed:?}, proved: {proved:?}")]
    UnexpectedRomDigest {
        preprocessed: RomDigest,
        proved: RomDigest,
    },
}

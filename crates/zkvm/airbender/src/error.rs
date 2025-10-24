use crate::client::VkHashChain;
use ere_zkvm_interface::zkVMError;
use std::{
    io,
    path::{Path, PathBuf},
    process::ExitStatus,
};
use thiserror::Error;

impl From<AirbenderError> for zkVMError {
    fn from(value: AirbenderError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum AirbenderError {
    // Compilation
    #[error(transparent)]
    CompileError(#[from] ere_compile_utils::CompileError),
    #[error("Failed to execute `rust-objcopy`: {0}")]
    RustObjcopy(#[source] io::Error),
    #[error("`rust-objcopy` failed with status: {status}\nstderr: {stderr}")]
    RustObjcopyFailed { status: ExitStatus, stderr: String },
    #[error("Failed to write ELF to `rust-objcopy` stdin: {0}")]
    RustObjcopyStdin(#[source] io::Error),

    // IO and file system
    #[error("IO failure: {0}")]
    Io(#[from] io::Error),
    #[error("IO failure in temporary directory: {0}")]
    TempDir(io::Error),

    // Serialization
    #[error("Bincode encode failed: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    #[error("Bincode decode failed: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),
    #[error("JSON deserialization failed: {0}")]
    JsonDeserialize(#[from] serde_json::Error),

    // Execution
    #[error("Failed to execute `airbender-cli run`: {0}")]
    AirbenderRun(#[source] io::Error),
    #[error("`airbender-cli run` failed with status: {status}\nstderr: {stderr}")]
    AirbenderRunFailed { status: ExitStatus, stderr: String },
    #[error("Failed to parse public value from stdout: {0}")]
    ParsePublicValue(String),
    #[error("Failed to parse cycles from stdout: {0}")]
    ParseCycles(String),

    // Proving
    #[error("Failed to execute `airbender-cli prove`: {0}")]
    AirbenderProve(#[source] io::Error),
    #[error("`airbender-cli prove` failed with status: {status}\nstderr: {stderr}")]
    AirbenderProveFailed { status: ExitStatus, stderr: String },
    #[error("Failed to execute `airbender-cli prove-final`: {0}")]
    AirbenderProveFinal(#[source] io::Error),
    #[error("`airbender-cli prove-final` failed with status: {status}\nstderr: {stderr}")]
    AirbenderProveFinalFailed { status: ExitStatus, stderr: String },
    #[error("Recursion proof not found at {path}")]
    RecursionProofNotFound { path: PathBuf },
    #[error("Final proof not found at {path}")]
    FinalProofNotFound { path: PathBuf },

    // Verification
    #[error("Proof verification failed")]
    ProofVerificationFailed,
    #[error("Invalid final register count, expected 32 but got {0}")]
    InvalidRegisterCount(usize),
    #[error(
        "Unexpected verification key hash chain - preprocessed: {preprocessed:?}, proved: {proved:?}"
    )]
    UnexpectedVkHashChain {
        preprocessed: VkHashChain,
        proved: VkHashChain,
    },
}

impl AirbenderError {
    pub fn io(err: io::Error, context: impl Into<String>) -> Self {
        Self::Io(io::Error::other(format!("{}: {}", context.into(), err)))
    }

    pub fn create_dir(err: io::Error, id: &str, path: impl AsRef<Path>) -> Self {
        let ctx = format!("Failed to create dir {id} at {}", path.as_ref().display());
        Self::io(err, ctx)
    }

    pub fn write_file(err: io::Error, id: &str, path: impl AsRef<Path>) -> Self {
        let ctx = format!("Failed to write {id} to {}", path.as_ref().display());
        Self::io(err, ctx)
    }

    pub fn read_file(err: io::Error, id: &str, path: impl AsRef<Path>) -> Self {
        let ctx = format!("Failed to read {id} from {}", path.as_ref().display());
        Self::io(err, ctx)
    }
}

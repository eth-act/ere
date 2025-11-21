use crate::zkvm::sdk::RomDigest;
use bytemuck::PodCastError;
use ere_zkvm_interface::zkvm::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Execution
    #[error("Total steps not found in execution report")]
    TotalStepsNotFound,

    // Rom setup
    #[error("Failed to find ROM digest in output")]
    RomDigestNotFound,

    #[error("`cargo-zisk rom-setup` failed in another thread")]
    RomSetupFailedBefore,

    // Prove
    #[error("Mutex of ZiskServer is poisoned")]
    MutexPoisoned,

    #[error("Server crashed")]
    ServerCrashed,

    #[error("Timeout waiting for server ready")]
    TimeoutWaitingServerReady,

    #[error("Uknown server status, stdout: {stdout}")]
    UnknownServerStatus { stdout: String },

    // Verify
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

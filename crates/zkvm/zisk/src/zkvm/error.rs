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

    #[error("Timeout waiting for server proving")]
    TimeoutWaitingServerProving,

    #[error("Timeout waiting for server ready")]
    TimeoutWaitingServerReady,

    #[error("Uknown server status, stdout: {stdout}")]
    UnknownServerStatus { stdout: String },

    // Network prove
    #[error("Failed to create tokio runtime: {0}")]
    TokioRuntimeCreation(String),

    #[error("Invalid coordinator URL: {0}")]
    InvalidCoordinatorUrl(String),

    #[error("Failed to connect to coordinator: {0}")]
    CoordinatorConnection(String),

    #[error("LaunchProof RPC failed: {0}")]
    LaunchProofRpc(String),

    #[error("Coordinator returned error: {0}")]
    CoordinatorError(String),

    #[error("SubscribeToProof RPC failed: {0}")]
    SubscribeToProofRpc(String),

    #[error("Proof stream error: {0}")]
    ProofStreamError(String),

    #[error("Proof job failed: {0}")]
    ProofJobFailed(String),

    #[error("Unknown proof status: {0}")]
    UnknownProofStatus(i32),

    #[error("Stream ended without completion status")]
    StreamEndedPrematurely,

    #[error("No proof data received")]
    NoProofData,

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

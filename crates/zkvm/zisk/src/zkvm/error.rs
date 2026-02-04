use crate::zkvm::sdk::RomDigest;
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
    #[error("Server crashed")]
    ServerCrashed,

    #[error("Timeout waiting for server proving")]
    TimeoutWaitingServerProving,

    #[error("Timeout waiting for server ready")]
    TimeoutWaitingServerReady,

    #[error("Unknown server status, stdout: {stdout}")]
    UnknownServerStatus { stdout: String },

    // Cluster
    #[error("Invalid cluster endpoint: {0}")]
    InvalidClusterEndpoint(String),

    #[error("Cluster gRPC error: {0}")]
    ClusterGrpcError(#[from] tonic::Status),

    #[error("Failed to connect to cluster: {0}")]
    ClusterConnectionFailed(String),

    #[error("Cluster error: {0}")]
    ClusterError(String),

    // Verify
    #[error("Invalid proof")]
    InvalidProof,

    #[error("Invalid proof size {0}, expected a multiple of 8")]
    InvalidProofSize(usize),

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

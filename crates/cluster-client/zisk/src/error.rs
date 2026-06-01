use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid cluster endpoint: {0}")]
    InvalidEndpoint(#[from] http::uri::InvalidUri),

    #[error("Cluster gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("Failed to connect to cluster: {0}")]
    ConnectionFailed(#[from] tonic::transport::Error),

    #[error("Cluster job {job_id} failed: {reason}")]
    JobFailed { job_id: String, reason: String },

    #[error("Cluster job {0} was cancelled")]
    JobCancelled(String),

    #[error("Cluster unavailable timed out")]
    ClusterUnavailable,

    #[error("Setup job {job_id} timed out")]
    SetupTimeout { job_id: String },

    #[error("Prove job {job_id} timed out")]
    ProveTimeout { job_id: String },

    #[error("Cluster response missing field: {0}")]
    MissingField(&'static str),

    #[error("Decode cluster proof failed: {0}")]
    DecodeProof(#[from] bincode::error::DecodeError),

    #[error(transparent)]
    Verifier(#[from] ere_verifier_zisk::Error),
}

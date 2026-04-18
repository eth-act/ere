use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid cluster endpoint: {0}")]
    InvalidEndpoint(#[from] http::uri::InvalidUri),

    #[error("Cluster gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("Failed to connect to cluster: {0}")]
    ConnectionFailed(#[from] tonic::transport::Error),

    #[error("Cluster error: {0}")]
    Cluster(String),

    #[error("Invalid proof format: {0}")]
    InvalidProofFormat(String),
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Cluster request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Cluster returned {status} for {path}: {body}")]
    Status {
        path: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("Cluster is busy with another proof")]
    ClusterBusy,

    #[error("Program {program} is not registered with the cluster")]
    ProgramNotRegistered { program: String },

    #[error("Cluster stack not ready: {0}")]
    NotReady(String),

    #[error("Create prove job timeout")]
    CreateProveJobTimeout,

    #[error("Prove job {proof_uuid} timed out")]
    ProveTimeout { proof_uuid: String },

    #[error("Prove job {proof_uuid} failed: {reason}")]
    JobFailed { proof_uuid: String, reason: String },

    #[error("Prove job {proof_uuid} was cancelled")]
    JobCancelled { proof_uuid: String },

    #[error("Unsupported input: {0}")]
    UnsupportedInput(&'static str),

    #[error("Cluster response missing field: {0}")]
    MissingField(&'static str),

    #[error("Failed to decode the program vk: {0}")]
    DecodeProgramVk(String),

    #[error("Failed to encode the program input: {0}")]
    EncodeInput(#[from] bincode::error::EncodeError),

    #[error("Failed to decode the proof status event {0}: {1}")]
    DecodeEvent(String, #[source] serde_json::Error),

    #[error("Proof status event stream failed: {0}")]
    EventStream(String),

    #[error(transparent)]
    Verifier(#[from] ere_verifier_openvm::Error),
}

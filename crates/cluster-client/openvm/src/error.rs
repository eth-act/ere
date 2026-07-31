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
    #[error("Program {program} is registered with different bytes")]
    ProgramConflict { program: String },
    #[error("No workers are registered with the cluster")]
    NoWorkers,
    #[error("Cluster stack not ready: {0}")]
    NotReady(String),
    #[error("Registering program {program} timed out")]
    RegisterTimeout { program: String },
    #[error("Create prove job timeout")]
    CreateProveJobTimeout,
    #[error("Prove job {proof_uuid} timed out")]
    ProveTimeout { proof_uuid: String },
    #[error("Prove job {proof_uuid} failed: {reason}")]
    JobFailed { proof_uuid: String, reason: String },
    #[error("Prove job {proof_uuid} was canceled")]
    JobCanceled { proof_uuid: String },
    #[error("Cluster response missing field: {0}")]
    MissingField(&'static str),
    #[error("Failed to derive the program vk from the ELF: {0}")]
    DeriveProgramVk(String),
    #[error("Failed to decode the program vk: {0}")]
    DecodeProgramVk(#[from] bincode::error::DecodeError),
    #[error("Failed to decode the proof status event {0}: {1}")]
    DecodeEvent(String, #[source] serde_json::Error),
    #[error("Proof status event stream failed: {0}")]
    EventStream(String),
    #[error(transparent)]
    Verifier(#[from] ere_verifier_openvm::Error),
}

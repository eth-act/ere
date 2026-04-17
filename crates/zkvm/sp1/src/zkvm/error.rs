use ere_zkvm_interface::zkvm::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Failed to setup ELF: {0}")]
    Setup(#[source] anyhow::Error),

    #[error("Deserialize proofs in Input failed: {0:?}")]
    DeserializeInputProofs(bincode::error::DecodeError),

    #[error("Missing `api_key` in `RemoteProverConfig`")]
    MissingApiKey,

    #[error("SP1 execution failed: {0}")]
    Execute(#[source] anyhow::Error),

    #[error("SP1 execution completed with non-success exit code: {0}")]
    ExecutionFailed(u32),

    #[error("SP1 SDK proving failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("Failed to extract exit code from proof")]
    ExitCodeExtractionFailed,

    #[error(transparent)]
    Verifier(#[from] ere_verifier_sp1::Error),
}

impl Error {
    pub fn setup(err: impl Into<anyhow::Error>) -> Self {
        Self::Setup(err.into())
    }

    pub fn prove(err: impl Into<anyhow::Error>) -> Self {
        Self::Prove(err.into())
    }
}

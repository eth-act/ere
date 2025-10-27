use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    CommonError(#[from] ere_compile_utils::CommonError),
}

#[derive(Debug, Error)]
pub enum PicoError {
    #[error(transparent)]
    CommonError(#[from] ere_zkvm_interface::CommonError),

    // Execute
    #[error("Pico execution panicked: {0}")]
    ExecutePanic(String),

    // Prove
    #[error("Pico proving failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("Pico proving panicked: {0}")]
    ProvePanic(String),

    // Verify
    #[error("Pico verifying failed: {0}")]
    Verify(#[source] anyhow::Error),

    #[error("Invalid base proof length {0}, expected 1")]
    InvalidBaseProofLength(usize),

    #[error("Invalid public values length {0}, expected at least 32")]
    InvalidPublicValuesLength(usize),

    #[error("First 32 public values are expected in byte")]
    InvalidPublicValues,

    #[error("Unexpected public value digest - claimed: {claimed:?}, proved: {proved:?}")]
    UnexpectedPublicValuesDigest { claimed: [u8; 32], proved: [u8; 32] },
}

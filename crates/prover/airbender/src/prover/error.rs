use ere_prover_core::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Execution
    #[error("Failed to parse public value from stdout: {0}")]
    ParsePublicValue(String),

    #[error("Failed to parse cycles from stdout: {0}")]
    ParseCycles(String),

    #[error(transparent)]
    Verifier(#[from] ere_verifier_airbender::Error),
}

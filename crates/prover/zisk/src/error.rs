use ere_prover_core::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
    #[error("Invalid env variable {key}, expected usize, got {value}")]
    InvalidEnvVar { key: &'static str, value: String },

    // Emulator
    #[error("ROM transpilation failed: {0}")]
    Riscv2zisk(String),

    #[error("Emulation not terminated")]
    EmulatorNotTerminated,

    #[error("Emulation failure")]
    EmulatorError,

    #[error("Emulator panicked: {0}")]
    EmulatorPanic(String),

    // SDK
    #[error("Build prover failed: {0}")]
    BuildProver(#[source] anyhow::Error),

    #[error("Setup failed: {0}")]
    Setup(#[source] anyhow::Error),

    #[error("Prove failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("Prove panicked: {0}")]
    ProvePanic(String),

    // Cluster
    #[error(transparent)]
    Cluster(#[from] ere_cluster_client_zisk::Error),

    // Verify
    #[error(transparent)]
    Verifier(#[from] ere_verifier_zisk::Error),
}

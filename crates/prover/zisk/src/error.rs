use ere_prover_core::CommonError;
use thiserror::Error;
use zisk_sm_rom::RomError;

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

    #[error("ZisK cost estimation failed: {0}")]
    EstimateCost(#[from] EstimateCostError),

    // SDK
    #[error("Build prover failed: {0}")]
    BuildProver(#[source] anyhow::Error),

    #[error("Build ROM failed: {0}")]
    BuildRom(#[from] RomError),

    #[error("Setup failed: {0}")]
    Setup(#[source] anyhow::Error),

    #[error("Prove failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("Prove panicked: {0}")]
    ProvePanic(String),

    #[error("Enable `cuda` feature to use `ProverResource::Gpu`")]
    CudaFeatureDisabled,

    // Cluster
    #[error(transparent)]
    Cluster(#[from] ere_cluster_client_zisk::Error),

    // Verify
    #[error(transparent)]
    Verifier(#[from] ere_verifier_zisk::Error),
}

#[derive(Debug, Error)]
pub enum EstimateCostError {
    #[error("emulator report is missing {0}")]
    MissingRows(String),

    #[error("components sum to {summed}, not the total {total}")]
    Mismatch { summed: u64, total: u64 },
}

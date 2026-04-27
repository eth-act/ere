use ere_prover_core::CommonError;
use proofman_common::ProofmanError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
    #[error("Invalid env variable {key}, expected usize, got {value}")]
    InvalidEnvVar { key: &'static str, value: String },

    #[error("Enable `cuda` feature to use `ProverResource::Gpu`")]
    CudaFeatureDisabled,

    #[error("Disable `cuda` feature to use `ProverResource::Cpu`")]
    CudaFeatureEnabled,

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
    #[error("Create ProofCtx failed: {0}")]
    ProofCtx(#[source] ProofmanError),

    #[error("Generate assembly failed: {0}")]
    GenerateAssembly(String),

    #[error("Compute ProgramVk failed: {0}")]
    ComputeProgramVk(#[source] anyhow::Error),

    #[error("Initialize prover failed: {0}")]
    InitProver(#[source] anyhow::Error),

    #[error("Setup prover failed: {0}")]
    SetupProver(#[source] anyhow::Error),

    #[error("Prove failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("Prove panicked: {0}")]
    ProvePanic(String),

    #[error("Expected VadcopFinal but got {0}")]
    UnexpectedProofKind(&'static str),

    #[error("Invalid proof format: {0}")]
    InvalidProofFormat(String),

    // Cluster
    #[error(transparent)]
    Cluster(#[from] ere_cluster_client_zisk::Error),

    // Verify
    #[error(transparent)]
    Verifier(#[from] ere_verifier_zisk::Error),
}

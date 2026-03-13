use crate::zkvm::sdk::ProgramVk;
use ere_zkvm_interface::zkvm::CommonError;
use proofman_common::ProofmanError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
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

    #[error("Compute program VK failed: {0}")]
    ComputeProgramVk(#[source] anyhow::Error),

    #[error("Invalid program VK length, expected 32, got {0}")]
    InvalidProgramVkLength(usize),

    #[error("Initialize prover failed: {0}")]
    InitProver(#[source] anyhow::Error),

    #[error("Initialize prover failed: {0}")]
    SetupProver(#[source] anyhow::Error),

    #[error("Prove failed: {0}")]
    Prove(#[source] anyhow::Error),

    #[error("Prove panicked: {0}")]
    ProvePanic(String),

    // Cluster
    #[error("Invalid cluster endpoint: {0}")]
    InvalidClusterEndpoint(#[from] http::uri::InvalidUri),

    #[error("Cluster gRPC error: {0}")]
    ClusterGrpcError(#[from] tonic::Status),

    #[error("Failed to connect to cluster: {0}")]
    ClusterConnectionFailed(#[from] tonic::transport::Error),

    #[error("Cluster error: {0}")]
    ClusterError(String),

    #[error("Invalid proof format: {0}")]
    InvalidProofFormat(anyhow::Error),

    // Verify
    #[error("Invalid proof")]
    InvalidProof,

    #[error("Invalid proof size {0}, expected a multiple of 8")]
    InvalidProofSize(usize),

    #[error("Unexpected program VK - preprocessed: {preprocessed:?}, proved: {proved:?}")]
    UnexpectedProgramVk {
        preprocessed: ProgramVk,
        proved: ProgramVk,
    },
}

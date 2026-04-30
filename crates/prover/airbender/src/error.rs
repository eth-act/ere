use airbender_host::HostError;
use ere_prover_core::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    #[error("Enable `cuda` feature to use `ProverResource::Gpu`")]
    CudaFeatureDisabled,

    #[error("Cpu prover not available, use `ProverResource::Gpu`")]
    CpuProverNotAvailable,

    #[error("Guest execution did not terminate")]
    ExecutionDidNotTerminate,

    #[error("Emulator panicked: {0}")]
    ExecutePanic(String),

    #[error(transparent)]
    Sdk(#[from] HostError),

    #[error(transparent)]
    Verifier(#[from] ere_verifier_airbender::Error),
}

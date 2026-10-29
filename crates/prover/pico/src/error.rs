use ere_prover_core::CommonError;
use pico_prover::ProveError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
    #[error("Enable `cuda` feature to enable `ProverResource::Gpu`")]
    CudaFeatureDisabled,

    #[error(
        "A build with the `cuda` feature proves on the GPU only; use one without it for `ProverResource::Cpu`"
    )]
    CpuUnsupportedWithCuda,

    #[error("Initialize prover failed: {0}")]
    ProverInit(#[source] ProveError),

    #[error("Pico prover panicked in an earlier call")]
    ProverPoisoned,

    // Execute
    #[error("Pico execution failed: {0}")]
    Execute(#[source] ProveError),

    // Estimate cost
    #[error("Pico cost estimation failed: {0}")]
    EstimateCost(#[source] ProveError),

    // Profile
    #[error("Pico does not support profiling yet")]
    ProfileUnsupported,

    // Prove
    #[error("Pico proving failed: {0}")]
    Prove(#[source] ProveError),

    #[error(transparent)]
    Verifier(#[from] ere_verifier_pico::Error),
}

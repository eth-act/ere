use ere_zkvm_interface::ProofKind;
use risc0_zkp::verify::VerificationError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    CommonError(#[from] ere_compile_utils::CommonError),

    #[error("`risc0_build::build_package` for {guest_path} failed: {err}")]
    BuildFailure {
        #[source]
        err: anyhow::Error,
        guest_path: PathBuf,
    },

    #[error("`risc0_build::build_package` succeeded but failed to find guest")]
    Risc0BuildMissingGuest,

    #[error("ELF binary image calculation failure: {0}")]
    ImageIDCalculationFailure(anyhow::Error),
}

#[derive(Debug, Error)]
pub enum Risc0Error {
    // Execute
    #[error("Failed to build `ExecutorEnv`: {0}")]
    BuildExecutorEnv(anyhow::Error),

    #[error("Failed to execute: {0}")]
    Execute(anyhow::Error),

    // Prove
    #[error("Failed to initialize cuda prover: {0}")]
    InitializeCudaProver(anyhow::Error),

    #[error("Failed to prove: {0}")]
    Prove(anyhow::Error),

    // Verify
    #[error("Invalid proof kind, expected: {0:?}, got: {1}")]
    InvalidProofKind(ProofKind, String),

    #[error("Failed to verify: {0}")]
    Verify(VerificationError),
}

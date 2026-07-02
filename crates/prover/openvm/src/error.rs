use ere_prover_core::CommonError;
use openvm_sdk::SdkError;
use openvm_transpiler::transpiler::TranspilerError;
use openvm_verify_stark_host::vk::VerificationBaseline;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),

    // Common
    #[error("Enable `cuda` feature to enable `ProverResource::Gpu`")]
    CudaFeatureDisabled,

    #[error("Decode elf failed: {0}")]
    DecodeElf(eyre::Report),

    #[error("Transpile elf failed: {0}")]
    Transpile(TranspilerError),

    #[error("Read internal_recursive_pk failed: {0}")]
    ReadInternalRecursivePkFailed(eyre::Error),

    #[error("Initialize prover failed: {0}")]
    ProverInit(SdkError),

    // Execute
    #[error("OpenVM execution failed: {0}")]
    Execute(#[source] SdkError),

    // Prove
    #[error("OpenVM proving failed: {0}")]
    Prove(#[source] SdkError),

    #[error("verification baseline mismatch: expected {expected:?}, got {proved:?}")]
    UnexpectedBaseline {
        expected: Box<VerificationBaseline>,
        proved: Box<VerificationBaseline>,
    },

    #[error(transparent)]
    Verifier(#[from] ere_verifier_openvm::Error),
}

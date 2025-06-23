use std::path::Path;
use thiserror::Error;

mod input;
pub use input::{Input, InputItem};

mod reports;
pub use reports::{ProgramExecutionReport, ProgramProvingReport};

mod network;
pub use network::NetworkProverConfig;

#[allow(non_camel_case_types)]
/// Compiler trait for compiling programs into an opaque sequence of bytes.
pub trait Compiler {
    type Error: std::error::Error + Send + Sync + 'static;
    type Program: Clone + Send + Sync;

    /// Compiles the program and returns the program
    fn compile(path_to_program: &Path) -> Result<Self::Program, Self::Error>;
}

/// ResourceType specifies what resource will be used to create the proofs.
#[derive(Debug, Clone, Default)]
pub enum ProverResourceType {
    #[default]
    Cpu,
    Gpu,
    /// Use a remote prover network
    Network(NetworkProverConfig),
}

/// An error that can occur during prove, execute or verification
/// of a zkVM.
///
/// Note: We use a concrete error type here, so that downstream crates
/// can do patterns such as Vec<dyn zkVM>
#[allow(non_camel_case_types)]
#[derive(Debug, Error)]
pub enum zkVMError {
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(String),

    /// Authentication error
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Timeout error
    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Service unavailable
    #[error("Prover service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Invalid response from network
    #[error("Invalid response from prover network: {0}")]
    InvalidResponse(String),

    // TODO: We can add more variants as time goes by.
    // TODO: for now, we use this catch-all as a way to prototype faster
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[allow(non_camel_case_types)]
#[auto_impl::auto_impl(&, Arc, Box)]
/// zkVM trait to abstract away the differences between each zkVM
pub trait zkVM {
    /// Executes the given program with the inputs accumulated in the Input struct.
    /// For RISCV programs, `program_bytes` will be the ELF binary
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, zkVMError>;

    /// Creates a proof for a given program
    fn prove(&self, inputs: &Input) -> Result<(Vec<u8>, ProgramProvingReport), zkVMError>;

    /// Verifies a proof for the given program
    /// TODO: Pass public inputs too and check that they match if they come with the
    /// TODO: proof, or append them if they do not.
    /// TODO: We can also just have this return the public inputs, but then the user needs
    /// TODO: ensure they check it for correct #[must_use]
    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError>;
}

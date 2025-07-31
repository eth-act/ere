use serde::{Serialize, de::DeserializeOwned};
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
    type Program: Clone + Send + Sync + Serialize + DeserializeOwned;

    /// Compiles the program and returns the program
    ///
    /// # Arguments
    /// * `mount_directory` - The base directory (workspace root)
    /// * `guest_relative` - The relative path from mount_directory to the guest program
    fn compile(mount_directory: &Path, guest_relative: &Path)
    -> Result<Self::Program, Self::Error>;
}

/// ResourceType specifies what resource will be used to create the proofs.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "clap", derive(clap::Subcommand))]
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
/// zkVM trait to abstract away the differences between each zkVM.
///
/// This trait provides a unified interface, the workflow is:
/// 1. Compile a guest program using the corresponding `Compiler`.
/// 2. Create a zkVM instance with the compiled program and prover resource.
/// 3. Execute, prove, and verify using the zkVM instance methods.
///
/// Note that a zkVM instance is created for specific program, each zkVM
/// implementation will have their own construction function.
pub trait zkVM {
    /// Executes the program with the provided inputs.
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, zkVMError>;

    /// Creates a proof of the program execution with given inputs.
    fn prove(&self, inputs: &Input) -> Result<(Vec<u8>, ProgramProvingReport), zkVMError>;

    /// Verifies a proof of the program used to create this zkVM instance.
    /// TODO: Pass public inputs too and check that they match if they come with the
    /// TODO: proof, or append them if they do not.
    /// TODO: We can also just have this return the public inputs, but then the user needs
    /// TODO: ensure they check it for correct #[must_use]
    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError>;

    /// Returns the name of the zkVM
    fn name(&self) -> &'static str;

    /// Returns the version of the zkVM SDK (e.g. 0.1.0)
    fn sdk_version(&self) -> &'static str;
}

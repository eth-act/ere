#![allow(non_camel_case_types)]

use serde::{Serialize, de::DeserializeOwned};

mod error;
mod proof;
mod report;
mod resource;

pub use error::CommonError;
pub use proof::{Proof, ProofKind};
pub use report::{ProgramExecutionReport, ProgramProvingReport};
pub use resource::{NetworkProverConfig, ProverResourceType};

/// Public values committed/revealed by guest program.
///
/// Use [`zkVM::deserialize_from`] to deserialize object from the bytes.
pub type PublicValues = Vec<u8>;

/// zkVM trait to abstract away the differences between each zkVM.
///
/// This trait provides a unified interface, the workflow is:
/// 1. Compile a guest program using the corresponding `Compiler`.
/// 2. Create a zkVM instance with the compiled program and prover resource.
/// 3. Execute, prove, and verify using the zkVM instance methods.
///
/// Note that a zkVM instance is created for specific program, each zkVM
/// implementation will have their own construction function.
#[auto_impl::auto_impl(&, Arc, Box)]
pub trait zkVM {
    /// Executes the program with the given input.
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)>;

    /// Creates a proof of the program execution with given input.
    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)>;

    /// Verifies a proof of the program used to create this zkVM instance, then
    /// returns the public values extracted from the proof.
    #[must_use = "Public values must be used"]
    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues>;

    /// Returns the name of the zkVM
    fn name(&self) -> &'static str;

    /// Returns the version of the zkVM SDK (e.g. 0.1.0)
    fn sdk_version(&self) -> &'static str;
}

pub trait zkVMProgramDigest {
    /// Digest of specific compiled guest program used when verify a proof.
    type ProgramDigest: Clone + Serialize + DeserializeOwned;

    /// Returns [`zkVMProgramDigest::ProgramDigest`].
    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest>;
}

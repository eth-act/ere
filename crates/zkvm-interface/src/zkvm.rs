#![allow(non_camel_case_types)]

use bincode::error::{DecodeError, EncodeError};
use serde::{Serialize, de::DeserializeOwned};

mod error;
mod proof;
mod report;
mod resource;

pub use error::CommonError;
pub use proof::{Proof, ProofKind};
pub use report::{ProgramExecutionReport, ProgramProvingReport};
pub use resource::{NetworkProverConfig, ProverResourceType};

/// Input for the prover to execute/prove a guest program.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub stdin: Vec<u8>,
    pub proofs: Option<Vec<u8>>,
}

impl Input {
    /// Creates a new `Input` with the given stdin.
    pub fn new(stdin: Vec<u8>) -> Self {
        Self {
            stdin,
            proofs: None,
        }
    }

    /// Returns a reference to the stdin as a byte slice.
    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }

    /// Deserializes and returns the proofs if present.
    ///
    /// # Returns
    ///
    /// - `None` if no proofs is set
    /// - `Some(Ok(..))` if the proofs was successfully deserialized
    /// - `Some(Err(..))` if deserialization failed
    pub fn proofs<T: DeserializeOwned>(&self) -> Option<Result<Vec<T>, DecodeError>> {
        self.proofs.as_ref().map(|proofs| {
            bincode::serde::decode_from_slice(proofs, bincode::config::legacy())
                .map(|(proofs, _)| proofs)
        })
    }

    /// Consumes `self` and returns a new `Input` with the serialized proofs, or
    /// returns an error if serialization failed.
    pub fn with_proof<T: Serialize>(mut self, proofs: &[T]) -> Result<Self, EncodeError> {
        self.proofs = Some(bincode::serde::encode_to_vec(
            proofs,
            bincode::config::legacy(),
        )?);
        Ok(self)
    }
}

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
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)>;

    /// Creates a proof of the program execution with given input.
    fn prove(
        &self,
        input: &Input,
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

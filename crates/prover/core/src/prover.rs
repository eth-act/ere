#![allow(non_camel_case_types)]

use std::error::Error;

use bincode::error::{DecodeError, EncodeError};
use serde::{Serialize, de::DeserializeOwned};

mod error;
mod report;
mod resource;

#[cfg(feature = "tokio")]
mod tokio;

pub use ere_verifier_core::{PublicValues, zkVMVerifier};
pub use error::CommonError;
pub use report::{ProgramExecutionReport, ProgramProvingReport};
pub use resource::{ProverResource, ProverResourceKind, RemoteProverConfig};

#[cfg(feature = "tokio")]
pub use tokio::block_on;

/// Input for the prover to execute/prove a guest program.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub stdin: Vec<u8>,
    /// Serialized proofs to be verified in guest program for proof composition.
    pub proofs: Option<Vec<u8>>,
}

impl Input {
    /// Creates a new `Input` with the empty stdin.
    pub fn new() -> Self {
        Self {
            stdin: Vec::new(),
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
    /// - `None` if no proofs are set
    /// - `Some(Ok(..))` if the proofs were successfully deserialized
    /// - `Some(Err(..))` if deserialization failed
    pub fn proofs<T: DeserializeOwned>(&self) -> Option<Result<Vec<T>, DecodeError>> {
        self.proofs.as_ref().map(|proofs| {
            bincode::serde::decode_from_slice(proofs, bincode::config::legacy())
                .map(|(proofs, _)| proofs)
        })
    }

    /// Sets stdin and returns a new `Input`.
    pub fn with_stdin(mut self, stdin: Vec<u8>) -> Self {
        self.stdin = stdin;
        self
    }

    /// Sets stdin with a LE u32 length prefix and returns a new `Input`.
    ///
    /// The `Platform::read_whole_input` requires stdin to have a LE u32 length
    /// prefix for efficiency reason.
    pub fn with_prefixed_stdin(mut self, stdin: Vec<u8>) -> Self {
        self.stdin = Vec::with_capacity(4 + stdin.len());
        self.stdin.extend((stdin.len() as u32).to_le_bytes());
        self.stdin.extend(stdin);
        self
    }

    /// Serializes the given proofs and returns a new `Input` with them set.
    ///
    /// Consumes `self` and returns an error if serialization fails.
    pub fn with_proofs<T: Serialize>(mut self, proofs: &[T]) -> Result<Self, EncodeError> {
        self.proofs = Some(bincode::serde::encode_to_vec(
            proofs,
            bincode::config::legacy(),
        )?);
        Ok(self)
    }

    /// Sets serialized proofs and returns a new `Input`.
    ///
    /// The proofs must be serialized using [`bincode::serde`] with
    /// [`bincode::config::legacy`] configuration.
    pub fn with_serialized_proofs(mut self, proofs: Vec<u8>) -> Self {
        self.proofs = Some(proofs);
        self
    }
}

/// zkVMProver trait to abstract away the differences between each zkVMProver.
///
/// This trait provides a unified interface, the workflow is:
/// 1. Compile a guest program using the corresponding `Compiler`.
/// 2. Create a zkVMProver instance with the compiled program and prover resource.
/// 3. Execute, prove, and verify using the zkVMProver instance methods.
///
/// Note that a zkVMProver instance is created for specific program, each zkVMProver
/// implementation will have their own construction function.
#[auto_impl::auto_impl(&, Arc, Box)]
pub trait zkVMProver {
    type Verifier: zkVMVerifier;
    type Error: 'static + Send + Sync + Error + From<<Self::Verifier as zkVMVerifier>::Error>;

    /// Returns a reference to the verifier.
    fn verifier(&self) -> &Self::Verifier;

    /// Executes the program with the given input.
    fn execute(&self, input: &Input)
    -> Result<(PublicValues, ProgramExecutionReport), Self::Error>;

    /// Creates a proof of the program execution with given input.
    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, Proof<Self>, ProgramProvingReport), Self::Error>;

    /// Verifies a proof of the program used to create this zkVMProver instance, then
    /// returns the public values extracted from the proof.
    #[must_use = "Public values must be used"]
    fn verify(&self, proof: &Proof<Self>) -> Result<PublicValues, Self::Error> {
        Ok(self.verifier().verify(proof)?)
    }

    /// Returns the verifying key for the specific program.
    fn program_vk(&self) -> &ProgramVk<Self> {
        self.verifier().program_vk()
    }

    /// Returns the name of the zkVMProver.
    fn name(&self) -> &'static str {
        self.verifier().name()
    }

    /// Returns the version of the zkVMProver SDK (e.g. 0.1.0).
    fn sdk_version(&self) -> &'static str {
        self.verifier().sdk_version()
    }
}

/// [`zkVMVerifier::Proof`] of [`zkVMProver::Verifier`].
pub type Proof<T> = <<T as zkVMProver>::Verifier as zkVMVerifier>::Proof;

/// [`zkVMVerifier::ProgramVk`] of [`zkVMProver::Verifier`].
pub type ProgramVk<T> = <<T as zkVMProver>::Verifier as zkVMVerifier>::ProgramVk;

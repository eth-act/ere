use core::error::Error;

use crate::{Input, ProgramExecutionReport, ProgramProvingReport, PublicValues, zkVMVerifier};

/// zkVM prover trait to abstract away the differences between each zkVM.
///
/// This trait provides a unified interface, the workflow is:
/// 1. Compile a guest program using the corresponding `Compiler`.
/// 2. Create a zkVM prover instance with the compiled ELF and prover resource.
/// 3. Execute, prove, and verify using the zkVM instance methods.
///
/// Note that a zkVM prover instance is created for specific program, each zkVM prover
/// implementation will have their own construction function.
#[allow(non_camel_case_types)]
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

    /// Verifies a proof of the program used to create this zkVM prover instance, then
    /// returns the public values extracted from the proof.
    #[must_use = "Public values must be used"]
    fn verify(&self, proof: &Proof<Self>) -> Result<PublicValues, Self::Error> {
        Ok(self.verifier().verify(proof)?)
    }

    /// Returns the verifying key for the specific program.
    fn program_vk(&self) -> &ProgramVk<Self> {
        self.verifier().program_vk()
    }

    /// Returns the name of the zkVM.
    fn name(&self) -> &'static str {
        self.verifier().name()
    }

    /// Returns the version of the zkVM SDK (e.g. 0.1.0).
    fn sdk_version(&self) -> &'static str {
        self.verifier().sdk_version()
    }
}

/// [`zkVMVerifier::Proof`] of [`zkVMProver::Verifier`].
pub type Proof<T> = <<T as zkVMProver>::Verifier as zkVMVerifier>::Proof;

/// [`zkVMVerifier::ProgramVk`] of [`zkVMProver::Verifier`].
pub type ProgramVk<T> = <<T as zkVMProver>::Verifier as zkVMVerifier>::ProgramVk;

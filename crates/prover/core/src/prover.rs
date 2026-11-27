use core::error::Error;
use std::time::Duration;

use crate::{
    Elf, ExitCode, Input, PublicValues,
    cost::{CostEstimation, CostProfile},
    zkVMVerifier,
};

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
#[auto_impl::auto_impl(Box)]
pub trait zkVMProver: Sync {
    type Verifier: zkVMVerifier;
    type Error: 'static + Send + Sync + Error + From<<Self::Verifier as zkVMVerifier>::Error>;

    /// Builds what the prover builds for the current program, without starting the prover, so a
    /// program that the prover cannot build fails here. A backend keeps the result in its disk
    /// cache when it has one. Backends that build nothing for a program do nothing.
    fn preprocess(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Returns a reference to the verifier.
    fn verifier(&self) -> &Self::Verifier;

    /// Switches to the program `elf` and drops the old one. On error, the old one stays.
    fn setup(&mut self, elf: Elf) -> Result<(), Self::Error>;

    /// Executes the program with the given input.
    fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Self::Error>;

    /// Executes the program and estimates the cost of proving that run. It does not prove.
    fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Self::Error>;

    /// Executes the program and splits the cost of [`Self::execute_estimated_cost`] over the guest
    /// call stacks, and measures the peak memory use. It does not prove.
    fn profile(&self, input: &Input) -> Result<(PublicValues, CostProfile), Self::Error>;

    /// Creates a proof of the program execution with given input.
    ///
    /// It returns an error for an execution that terminates with a non-zero exit code.
    fn prove(&self, input: &Input) -> Result<(PublicValues, Proof<Self>, Duration), Self::Error>;

    /// Creates a proof of the program execution with given input, and returns the exit code of
    /// the execution with it.
    ///
    /// A zkVM that can prove a non-zero exit code also proves an execution that terminates with a
    /// non-zero exit code. Other zkVMs return an error for it, as [`Self::prove`] does.
    fn prove_any_exit_code(
        &self,
        input: &Input,
    ) -> Result<(ExitCode, PublicValues, Proof<Self>, Duration), Self::Error>;

    /// Verifies a proof of the current program of this zkVM prover instance, then
    /// returns the public values extracted from the proof.
    #[must_use = "Public values must be used"]
    fn verify(&self, proof: &Proof<Self>) -> Result<PublicValues, Self::Error> {
        Ok(self.verifier().verify(proof)?)
    }

    /// Verifies a proof of the current program of this zkVM prover instance, then returns the
    /// exit code and the public values extracted from the proof.
    #[must_use = "Exit code and public values must be used"]
    fn verify_any_exit_code(
        &self,
        proof: &Proof<Self>,
    ) -> Result<(ExitCode, PublicValues), Self::Error> {
        Ok(self.verifier().verify_any_exit_code(proof)?)
    }

    /// Returns the verifying key for the specific program.
    fn program_vk(&self) -> &ProgramVk<Self> {
        self.verifier().program_vk()
    }

    /// Returns the name of the zkVM.
    fn name(&self) -> &'static str {
        self.verifier().name()
    }

    /// Returns the version of the zkVM SDK (e.g. v0.1.0).
    fn sdk_version(&self) -> &'static str {
        self.verifier().sdk_version()
    }
}

/// [`zkVMVerifier::Proof`] of [`zkVMProver::Verifier`].
pub type Proof<T> = <<T as zkVMProver>::Verifier as zkVMVerifier>::Proof;

/// [`zkVMVerifier::ProgramVk`] of [`zkVMProver::Verifier`].
pub type ProgramVk<T> = <<T as zkVMProver>::Verifier as zkVMVerifier>::ProgramVk;

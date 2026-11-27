use core::{error::Error, fmt::Debug};

use crate::{
    PublicValues,
    codec::{Decode, Encode},
};

/// Exit code of a guest execution, the C `int` that the entry point returns, as the [Execution
/// Termination Semantics] define. Exit code 0 is a successful termination.
///
/// [Execution Termination Semantics]: https://github.com/eth-act/zkevm-standards/blob/main/standards/standard-termination-semantics/README.md
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitCode(pub i32);

/// zkVM verifier trait.
///
/// Note that a zkVM verifier instance is created for specific program.
#[allow(non_camel_case_types)]
#[auto_impl::auto_impl(&, Arc, Box)]
pub trait zkVMVerifier: 'static + Send + Sync + Clone + Debug {
    type ProgramVk: 'static + Send + Sync + Encode + Decode;
    type Proof: 'static + Send + Sync + Encode + Decode;
    type Error: 'static + Send + Sync + Error;

    /// Verifies a proof of the program used to create this zkVM verifier instance, then
    /// returns the public values extracted from the proof.
    ///
    /// It accepts only a proof of an execution that terminated with exit code 0.
    #[must_use = "Public values must be used"]
    fn verify(&self, proof: &Self::Proof) -> Result<PublicValues, Self::Error>;

    /// Verifies a proof of the program used to create this zkVM verifier instance, then
    /// returns the exit code and the public values extracted from the proof.
    ///
    /// It accepts a proof of an execution that terminated with any exit code. The caller compares
    /// the exit code with the expected one. A zkVM that cannot prove a non-zero exit code returns
    /// exit code 0 for each proof that verifies.
    #[must_use = "Exit code and public values must be used"]
    fn verify_any_exit_code(
        &self,
        proof: &Self::Proof,
    ) -> Result<(ExitCode, PublicValues), Self::Error>;

    /// Returns the verifying key for the specific program.
    fn program_vk(&self) -> &Self::ProgramVk;

    /// Returns the name of the zkVM.
    fn name(&self) -> &'static str;

    /// Returns the version of the zkVM SDK (e.g. v0.1.0).
    fn sdk_version(&self) -> &'static str;
}

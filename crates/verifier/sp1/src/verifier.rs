use core::fmt;

use ere_util_tokio::block_on;
use ere_verifier_core::{PublicValues, zkVMVerifier};
use sp1_sdk::{LightProver, Prover, SP1Proof as SP1SdkProof};

use crate::{Error, SP1ProgramVk, SP1Proof};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`SP1ProgramVk`]
/// and a [`LightProver`] used to perform verification via the `sp1-sdk`
/// verification routine.
pub struct SP1Verifier {
    prover: LightProver,
    program_vk: SP1ProgramVk,
}

impl fmt::Debug for SP1Verifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SP1Verifier")
            .field("program_vk", &self.program_vk)
            .finish_non_exhaustive()
    }
}

impl SP1Verifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: SP1ProgramVk) -> Self {
        Self {
            prover: block_on(LightProver::new()),
            program_vk,
        }
    }
}

impl zkVMVerifier for SP1Verifier {
    type ProgramVk = SP1ProgramVk;
    type Proof = SP1Proof;
    type Error = Error;

    fn program_vk(&self) -> &SP1ProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &SP1Proof) -> Result<PublicValues, Error> {
        if !matches!(proof.0.proof, SP1SdkProof::Compressed(_)) {
            return Err(Error::UnexpectedProofKind((&proof.0.proof).into()));
        }

        self.prover.verify(&proof.0, &self.program_vk.0, None)?;

        Ok(proof.0.public_values.as_slice().into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

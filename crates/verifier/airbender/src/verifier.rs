use airbender_host::verify_proof;
use ere_verifier_core::{PublicValues, zkVMVerifier};

use crate::{AirbenderProgramVk, AirbenderProof, Error};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`AirbenderProgramVk`]
/// needed to authenticate proofs.
#[derive(Debug)]
pub struct AirbenderVerifier {
    program_vk: AirbenderProgramVk,
}

impl AirbenderVerifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: AirbenderProgramVk) -> Self {
        Self { program_vk }
    }
}

impl zkVMVerifier for AirbenderVerifier {
    type ProgramVk = AirbenderProgramVk;
    type Proof = AirbenderProof;
    type Error = Error;

    fn program_vk(&self) -> &AirbenderProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &AirbenderProof) -> Result<PublicValues, Error> {
        verify_proof(&proof.0, &self.program_vk.0, None, None)?;

        Ok(proof.public_values().into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

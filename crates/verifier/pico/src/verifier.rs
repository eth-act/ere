use ere_verifier_core::{PublicValues, zkVMVerifier};

use crate::{Error, PicoProgramVk, PicoProof};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the [`PicoProgramVk`], and returns the
/// committed bytes of a proof as its public values.
#[derive(Clone, Debug)]
pub struct PicoVerifier {
    program_vk: PicoProgramVk,
}

impl PicoVerifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: PicoProgramVk) -> Self {
        Self { program_vk }
    }
}

impl zkVMVerifier for PicoVerifier {
    type ProgramVk = PicoProgramVk;
    type Proof = PicoProof;
    type Error = Error;

    fn program_vk(&self) -> &PicoProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &PicoProof) -> Result<PublicValues, Error> {
        pico_verifier::verify_with(self.program_vk.apex(), &proof.envelope, &proof.committed)
            .map_err(Error::Verify)?;
        Ok(proof.committed.as_slice().into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

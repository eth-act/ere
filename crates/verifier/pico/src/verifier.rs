use core::fmt;
use std::sync::Arc;

use ere_verifier_core::{PublicValues, zkVMVerifier};
use pico_verifier::{MleApexVerifier, bundle::apex_from_bundle, shipped_vk_root};

use crate::{Error, PicoProgramVk, PicoProof};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the [`PicoProgramVk`] and the verifier built
/// from it, and returns the committed bytes of a proof as its public values.
#[derive(Clone)]
pub struct PicoVerifier {
    program_vk: PicoProgramVk,
    apex: Arc<MleApexVerifier>,
}

impl fmt::Debug for PicoVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PicoVerifier")
            .field("program_vk", &self.program_vk)
            .finish_non_exhaustive()
    }
}

impl PicoVerifier {
    /// Creates a new verifier bound to `program_vk`.
    ///
    /// Refuses a bundle whose whitelist root is not the one shipped with Pico, or
    /// whose memberships do not reach that root.
    pub fn new(program_vk: PicoProgramVk) -> Result<Self, Error> {
        let apex = apex_from_bundle(&program_vk.0, shipped_vk_root())
            .map_err(Error::UntrustedProgramVk)?;
        Ok(Self {
            program_vk,
            apex: Arc::new(apex),
        })
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
        pico_verifier::verify_with(&self.apex, &proof.envelope, &proof.committed)
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

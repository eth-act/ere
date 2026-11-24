use core::fmt;
use std::sync::Arc;

use ere_verifier_core::{PublicValues, zkVMVerifier};
use pico_mle_core::config::recursion_params_with_128_bits_security;
use pico_verifier::{
    Kb, MleApexVerifier, shipped_compress_vk, shipped_vk_root,
    whitelist::{VkIdentity, VkWhitelist, vk_identity},
};

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
    pub fn new(program_vk: PicoProgramVk) -> Self {
        let root = shipped_vk_root();
        let whitelist = ShippedCompressVk {
            root,
            identity: vk_identity(&shipped_compress_vk()),
        };
        let apex = MleApexVerifier::new(
            recursion_params_with_128_bits_security(),
            Arc::new(whitelist),
            program_vk.0,
            root,
        );
        Self {
            program_vk,
            apex: Arc::new(apex),
        }
    }
}

/// Whitelist that holds only the compress key shipped with Pico, the one key that
/// `verify_compress_with` authenticates.
struct ShippedCompressVk {
    root: [Kb; 8],
    identity: VkIdentity,
}

impl VkWhitelist for ShippedCompressVk {
    fn root(&self) -> [Kb; 8] {
        self.root
    }

    fn contains(&self, id: &VkIdentity) -> bool {
        *id == self.identity
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
        pico_verifier::verify_compress_with(
            &self.apex,
            &pico_verifier::shipped_compress_vk(),
            &proof.envelope,
            &proof.committed,
        )
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

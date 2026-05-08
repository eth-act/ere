use std::sync::LazyLock;

use airbender_execution_utils::unified_circuit::verify_proof_in_unified_layer;
use ere_verifier_core::{PublicValues, zkVMVerifier};

use crate::{
    AirbenderProgramVk, AirbenderProof, Error,
    proof::words_to_le_bytes,
    verifier::vk::{SECURITY, UNIFIED_VK},
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod vk;

/// Verifier bound to a specific compiled guest program.
#[derive(Debug)]
pub struct AirbenderVerifier {
    program_vk: AirbenderProgramVk,
}

impl AirbenderVerifier {
    pub fn new(program_vk: AirbenderProgramVk) -> Self {
        LazyLock::force(&UNIFIED_VK);
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
        let setup = &UNIFIED_VK.unified_setup;
        let layouts = &UNIFIED_VK.unified_layouts;
        let output = verify_proof_in_unified_layer(&proof.0, setup, layouts, false, SECURITY)
            .map_err(|_| Error::InvalidProof)?;

        let (&[public_values, hash_chain], _) = output.as_chunks() else {
            unreachable!()
        };

        if hash_chain != self.program_vk.0 {
            return Err(Error::UnexpectedHashChain {
                expected: self.program_vk.0,
                got: hash_chain,
            });
        }

        Ok(words_to_le_bytes(public_values).into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

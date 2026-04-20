use bytemuck::cast_slice;
use ere_verifier_core::{PublicValues, zkVMVerifier};
use proofman_verifier::verify_vadcop_final;

use crate::{Error, ZiskProgramVk, ZiskProof};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Verifying key of the aggregation proof.
const VADCOP_FINAL_VK: [u64; 4] = [
    9211010158316595036,
    7055235338110277438,
    2391371252028311145,
    10691781997660262077,
];

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`ZiskProgramVk`]
/// needed to authenticate proofs.
pub struct ZiskVerifier {
    program_vk: ZiskProgramVk,
}

impl ZiskVerifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: ZiskProgramVk) -> Self {
        Self { program_vk }
    }
}

impl zkVMVerifier for ZiskVerifier {
    type ProgramVk = ZiskProgramVk;
    type Proof = ZiskProof;
    type Error = Error;

    fn program_vk(&self) -> &ZiskProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &ZiskProof) -> Result<PublicValues, Self::Error> {
        let program_vk = proof.program_vk();
        if program_vk != self.program_vk {
            return Err(Error::UnexpectedProgramVk {
                expected: self.program_vk,
                got: program_vk,
            });
        }

        if !verify_vadcop_final(&proof.vadcop_final_proof()?, cast_slice(&VADCOP_FINAL_VK)) {
            return Err(Error::InvalidProof);
        }

        Ok(proof.public_values.as_slice().into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

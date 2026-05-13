use ere_verifier_core::{PublicValues, zkVMVerifier};
use proofman_verifier::verify_vadcop_final_compressed_u64;

use crate::{Error, ZiskProgramVk, ZiskProof, verifier::vk::VADCOP_FINAL_COMPRESSED_VK};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod vk;

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`ZiskProgramVk`]
/// needed to authenticate proofs.
#[derive(Clone, Copy, Debug)]
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
        let (program_vk, public_values) = proof.program_vk_and_public_values()?;

        ensure_program_vk_matches(self.program_vk, program_vk)?;

        if !verify_vadcop_final_compressed_u64(
            &proof.0.proof_with_publics(),
            &VADCOP_FINAL_COMPRESSED_VK,
        ) {
            return Err(Error::InvalidProof);
        }

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

/// Returns [`Error::UnexpectedProgramVk`] when a proof's embedded `program_vk` does not match the
/// one preprocessed at construction time.
pub fn ensure_program_vk_matches(expected: ZiskProgramVk, got: ZiskProgramVk) -> Result<(), Error> {
    if expected != got {
        return Err(Error::UnexpectedProgramVk { expected, got });
    }
    Ok(())
}

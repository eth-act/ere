use crate::{AirbenderProgramVk, AirbenderProof, Error};
use airbender_execution_utils::{ProgramProof, verify_recursion_log_23_layer};
use core::array::from_fn;
use ere_verifier_core::{PublicValues, zkVMVerifier};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`AirbenderProgramVk`]
/// needed to authenticate proofs.
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
        if !verify_recursion_log_23_layer(&proof.0) {
            return Err(Error::VerifyFailed);
        }

        let (public_values, program_vk) = extract_public_values_and_program_vk(&proof.0)?;

        if self.program_vk != program_vk {
            return Err(Error::UnexpectedProgramVk {
                expected: self.program_vk,
                got: program_vk,
            });
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

/// Extract public values and VK hash chain from register values.
pub fn extract_public_values_and_program_vk(
    proof: &ProgramProof,
) -> Result<(PublicValues, AirbenderProgramVk), Error> {
    if proof.register_final_values.len() != 32 {
        return Err(Error::InvalidRegisterCount(
            proof.register_final_values.len(),
        ));
    }

    let public_values = proof.register_final_values[10..18]
        .iter()
        .flat_map(|value| value.value.to_le_bytes())
        .collect::<Vec<u8>>();

    let vk_hash_chain = from_fn(|i| proof.register_final_values[18 + i].value);

    Ok((public_values.into(), AirbenderProgramVk(vk_hash_chain)))
}

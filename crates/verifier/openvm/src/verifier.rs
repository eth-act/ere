use ere_verifier_core::{PublicValues, zkVMVerifier};
use openvm_stark_sdk::{
    config::baby_bear_poseidon2::F, openvm_stark_backend::p3_field::PrimeField32,
};
use openvm_verify_stark_host::{verify_vm_stark_proof_decoded, vk::VmStarkVerifyingKey};

use crate::{Error, OpenVMProgramVk, OpenVMProof, verifier::vk::AGG_VK};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod vk;

/// Public values bytes of OpenVM proof.
pub const NUM_PUBLIC_VALUES_BYTES: usize = 256;

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`OpenVMProgramVk`]
/// and the aggregation verifying key embedded at build time needed to
/// authenticate proofs.
#[derive(Clone, Debug)]
pub struct OpenVMVerifier {
    program_vk: OpenVMProgramVk,
    vk: VmStarkVerifyingKey,
}

impl OpenVMVerifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: OpenVMProgramVk) -> Self {
        let vk = VmStarkVerifyingKey {
            mvk: AGG_VK.clone(),
            baseline: program_vk.0.clone(),
        };
        Self { program_vk, vk }
    }
}

impl zkVMVerifier for OpenVMVerifier {
    type ProgramVk = OpenVMProgramVk;
    type Proof = OpenVMProof;
    type Error = Error;

    fn program_vk(&self) -> &OpenVMProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &OpenVMProof) -> Result<PublicValues, Error> {
        verify_vm_stark_proof_decoded(&self.vk, &proof.0)?;

        extract_public_values(&proof.0.user_pvs_proof.public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

/// Extract public values in bytes from field elements.
///
/// The public values revealed in guest program will be flatten into `Vec<u8>`
/// then converted to field elements `Vec<F>`, one per little-endian `u16`
/// memory cell, so here we try to downcast and expand it.
pub fn extract_public_values(user_public_values: &[F]) -> Result<PublicValues, Error> {
    let public_values = user_public_values
        .iter()
        .map(|v| u16::try_from(v.as_canonical_u32()).ok())
        .collect::<Option<Vec<u16>>>()
        .ok_or(Error::InvalidPublicValue)?
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<u8>>();
    if public_values.len() != NUM_PUBLIC_VALUES_BYTES {
        return Err(Error::InvalidPublicValueSize {
            expected: NUM_PUBLIC_VALUES_BYTES,
            got: public_values.len(),
        });
    }
    Ok(PublicValues(public_values))
}

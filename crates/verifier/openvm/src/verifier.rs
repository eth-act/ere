use core::fmt;
use std::sync::LazyLock;

use ere_verifier_core::{PublicValues, zkVMVerifier};
use openvm_continuations::F;
use openvm_stark_sdk::openvm_stark_backend::p3_field::PrimeField32;

use crate::{
    Error, OpenVMProgramVk, OpenVMProof,
    vendor::{AggVerifyingKey, verify_proof},
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

static AGG_VK: LazyLock<AggVerifyingKey> =
    LazyLock::new(|| bitcode::deserialize(include_bytes!("../agg_stark.vk")).unwrap());

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`OpenVMProgramVk`]
/// and the aggregation verifying key embedded at build time needed to
/// authenticate proofs.
pub struct OpenVMVerifier {
    program_vk: OpenVMProgramVk,
}

impl fmt::Debug for OpenVMVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenVMVerifier")
            .field("program_vk", &self.program_vk)
            .finish_non_exhaustive()
    }
}

impl OpenVMVerifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: OpenVMProgramVk) -> Self {
        let _ = &*AGG_VK;
        Self { program_vk }
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
        verify_proof(&AGG_VK, self.program_vk.0, &proof.0)?;

        extract_public_values(&proof.0.user_public_values)
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
/// then converted to field elements `Vec<F>`, so here we try to downcast it.
fn extract_public_values(user_public_values: &[F]) -> Result<PublicValues, Error> {
    user_public_values
        .iter()
        .map(|v| u8::try_from(v.as_canonical_u32()).ok())
        .collect::<Option<Vec<u8>>>()
        .ok_or(Error::InvalidPublicValue)
        .map(PublicValues::from)
}

#[cfg(test)]
mod tests {
    use openvm_sdk::Sdk;

    use crate::verifier::AGG_VK;

    #[test]
    fn test_agg_vk_correctness() {
        assert_eq!(
            bitcode::serialize(&Sdk::standard().agg_keygen().unwrap().1).unwrap(),
            bitcode::serialize(&*AGG_VK).unwrap()
        );
    }
}

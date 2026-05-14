use std::sync::LazyLock;

use ere_verifier_core::{PublicValues, zkVMVerifier};
use sp1_verifier::compressed::SP1CompressedVerifier;

use crate::{Error, SP1ProgramVk, SP1Proof};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

static COMPRESSED_VERIFIER: LazyLock<SP1CompressedVerifier> =
    LazyLock::new(SP1CompressedVerifier::new);

/// Verifier bound to a specific compiled guest program.
#[derive(Clone, Copy, Debug)]
pub struct SP1Verifier {
    program_vk: SP1ProgramVk,
}

impl SP1Verifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: SP1ProgramVk) -> Self {
        LazyLock::force(&COMPRESSED_VERIFIER);
        Self { program_vk }
    }
}

impl zkVMVerifier for SP1Verifier {
    type ProgramVk = SP1ProgramVk;
    type Proof = SP1Proof;
    type Error = Error;

    fn program_vk(&self) -> &SP1ProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &SP1Proof) -> Result<PublicValues, Error> {
        let public_values = proof.0.public_values.as_slice();

        let Some(proof) = proof.0.proof.try_as_compressed_ref() else {
            return Err(Error::UnexpectedProofKind(proof.0.mode()));
        };

        COMPRESSED_VERIFIER.verify_compressed_with_public_values(
            proof,
            public_values,
            &self.program_vk.0,
        )?;

        Ok(public_values.into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

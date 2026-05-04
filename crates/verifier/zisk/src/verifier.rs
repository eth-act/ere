use bytemuck::cast_slice;
use ere_verifier_core::{PublicValues, zkVMVerifier};
use proofman_verifier::verify_vadcop_final_compressed_bytes;

use crate::{Error, ZiskProgramVk, ZiskProof};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Aggregation verifying key for VadcopFinalMinimal proofs in zisk v0.17.0.
///
/// To reproduce:
///
/// ```bash
/// python3 -c "import struct,sys; print(list(struct.unpack('<4Q',open(sys.argv[1],'rb').read())))" \
///     $HOME/.zisk/provingKey/zisk/vadcop_final_compressed/vadcop_final_compressed.verkey.bin
/// ```
const VADCOP_FINAL_MINIMAL_VK: [u64; 4] = [
    371850295254322978,
    2764832171281751502,
    14747498303081942412,
    8181136173693786776,
];

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`ZiskProgramVk`]
/// needed to authenticate proofs.
#[derive(Debug)]
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
        let (program_vk, public_values) = proof.to_parts()?;

        ensure_program_vk_matches(self.program_vk, program_vk)?;

        let proof_bytes = proof.as_bytes()?;
        let vk_bytes = cast_slice(&VADCOP_FINAL_MINIMAL_VK);
        if !verify_vadcop_final_compressed_bytes(proof_bytes, vk_bytes) {
            return Err(Error::InvalidProof);
        }

        Ok(public_values.into())
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

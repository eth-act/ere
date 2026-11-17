use bincode::{
    config::legacy,
    error::{DecodeError, EncodeError},
};
use ere_verifier_core::{
    PublicValues,
    codec::{Decode, Encode},
};
use serde::{Deserialize, Serialize};
pub use zisk_verifier::VadcopFinalProof;
use zisk_verifier::{PROGRAM_VK_LEN, ZISK_PUBLICS};

use crate::{Error, VADCOP_FINAL_HASH_FAMILY, ZiskProgramVk};

pub const PROGRAM_VK_WORDS: usize = PROGRAM_VK_LEN;
pub const PUBLIC_VALUES_WORDS: usize = ZISK_PUBLICS;
pub const PUBLIC_VALUES_BYTES: usize = 4 * PUBLIC_VALUES_WORDS;

/// Limit of the encoded proof size, which holds the 299,735 bytes of the largest compressed final
/// proof of the blake3 family.
const MAX_PROOF_BYTES: usize = 300 * 1024;

/// A proof produced by the host prover that bundles everything needed for
/// verification.
///
/// Wraps [`VadcopFinalProof`], serialized via bincode legacy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZiskProof(pub VadcopFinalProof);

impl ZiskProof {
    pub fn program_vk_and_public_values(&self) -> Result<(ZiskProgramVk, PublicValues), Error> {
        if !self.0.compressed {
            return Err(Error::InvalidVadcopFinalProofKind);
        }

        if self.0.hash != VADCOP_FINAL_HASH_FAMILY {
            return Err(Error::InvalidVadcopFinalProofKind);
        }

        if self.0.public_values.len() != PROGRAM_VK_LEN + ZISK_PUBLICS {
            return Err(Error::InvalidPublicValueLength {
                expected: PROGRAM_VK_LEN + ZISK_PUBLICS,
                got: self.0.public_values.len(),
            });
        }

        let program_vk = self.0.public_values[..PROGRAM_VK_LEN].try_into().unwrap();

        let public_values = self.0.public_values[PROGRAM_VK_LEN..]
            .iter()
            .map(|v| u32::try_from(*v).ok().map(|value| value.to_le_bytes()))
            .collect::<Option<Vec<_>>>()
            .ok_or(Error::InvalidPublicValue)?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        Ok((program_vk, public_values.into()))
    }
}

impl Encode for ZiskProof {
    type Error = EncodeError;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        bincode::serde::encode_to_vec(self, legacy())
    }
}

impl Decode for ZiskProof {
    type Error = DecodeError;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        let (proof, consumed) =
            bincode::serde::decode_from_slice(slice, legacy().with_limit::<MAX_PROOF_BYTES>())?;
        if consumed != slice.len() {
            return Err(DecodeError::Other("trailing bytes after decoded value"));
        }
        Ok(proof)
    }
}

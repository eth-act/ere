use ere_verifier_core::PublicValues;
pub use proofman_verifier::VadcopFinalProof;
use serde::{Deserialize, Serialize};
use zisk_verifier::{PROGRAM_VK_LEN, ZISK_PUBLICS};

use crate::{Error, ZiskProgramVk};

pub const PROGRAM_VK_WORDS: usize = PROGRAM_VK_LEN;
pub const PUBLIC_VALUES_WORDS: usize = ZISK_PUBLICS;
pub const PUBLIC_VALUES_BYTES: usize = 4 * PUBLIC_VALUES_WORDS;

/// Zisk VadcopFinalProof proof in u64 words.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZiskProof(pub VadcopFinalProof);

impl ZiskProof {
    pub fn program_vk_and_public_values(&self) -> Result<(ZiskProgramVk, PublicValues), Error> {
        if !self.0.compressed {
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

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(ZiskProof, reject_trailing_bytes);

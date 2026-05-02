use std::iter;

use bytemuck::cast_slice;
use serde::{Deserialize, Serialize};

use crate::{Error, ZiskProgramVk};

const PROGRAM_VK_OFFSET: usize = 1;
const PROGRAM_VK_WORDS: usize = 4;
const PUBLIC_VALUES_OFFSET: usize = PROGRAM_VK_OFFSET + PROGRAM_VK_WORDS;
const PUBLIC_VALUES_WORDS: usize = 64;
pub const PUBLIC_VALUES_BYTES: usize = 4 * PUBLIC_VALUES_WORDS;
const PROOF_PREFIX_WORDS: usize = PROGRAM_VK_WORDS + PUBLIC_VALUES_WORDS;
const PROOF_BODY_WORDS: usize = 32594;
const PROOF_BODY_BYTES: usize = 8 * PROOF_BODY_WORDS;
const PROOF_WORDS: usize = 1 + PROOF_PREFIX_WORDS + PROOF_BODY_WORDS;

/// Zisk VadcopFinalMinimal proof in u64 words.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZiskProof(pub Vec<u64>);

impl ZiskProof {
    /// Construct VadcopFinalMinimal proof in u64 words from parts.
    pub fn from_parts(
        program_vk: &ZiskProgramVk,
        public_values: &[u8; PUBLIC_VALUES_BYTES],
        proof_body: &[u8],
    ) -> Result<Self, Error> {
        if proof_body.len() != PROOF_BODY_BYTES {
            return Err(Error::InvalidProofFormat(format!(
                "proof body has {} bytes, expected to be {PROOF_BODY_BYTES}",
                proof_body.len()
            )));
        }

        let proof = iter::empty()
            .chain([PROOF_PREFIX_WORDS as u64])
            .chain(program_vk.0)
            .chain(
                public_values
                    .chunks_exact(4)
                    .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()) as u64),
            )
            .chain(
                proof_body
                    .chunks_exact(8)
                    .map(|bytes| u64::from_le_bytes(bytes.try_into().unwrap())),
            )
            .collect();

        Ok(Self(proof))
    }

    /// Returns the program verifying key and public values.
    pub fn to_parts(&self) -> Result<(ZiskProgramVk, [u8; PUBLIC_VALUES_BYTES]), Error> {
        self.validate_format()?;
        Ok((self.program_vk(), self.public_values()?))
    }

    /// Returns the proof in bytes to be verified by [`verify_vadcop_final_compressed_bytes`].
    ///
    /// [`verify_vadcop_final_compressed_bytes`]: proofman_verifier::verify_vadcop_final_compressed_bytes
    pub fn as_bytes(&self) -> Result<&[u8], Error> {
        self.validate_format()?;
        Ok(cast_slice(&self.0))
    }

    fn program_vk(&self) -> ZiskProgramVk {
        let words = &self.0[PROGRAM_VK_OFFSET..PROGRAM_VK_OFFSET + PROGRAM_VK_WORDS];
        ZiskProgramVk(words.try_into().unwrap())
    }

    fn public_values(&self) -> Result<[u8; PUBLIC_VALUES_BYTES], Error> {
        let mut bytes = [0u8; PUBLIC_VALUES_BYTES];
        let words = &self.0[PUBLIC_VALUES_OFFSET..PUBLIC_VALUES_OFFSET + PUBLIC_VALUES_WORDS];
        for (chunk, &word) in bytes.chunks_exact_mut(4).zip(words) {
            let word = u32::try_from(word).map_err(|_| {
                Error::InvalidProofFormat(
                    "public value words are expected to be in u32".to_string(),
                )
            })?;
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        Ok(bytes)
    }

    fn validate_format(&self) -> Result<(), Error> {
        if self.0.len() != PROOF_WORDS {
            return Err(Error::InvalidProofFormat(format!(
                "proof has {} u64 words, expected to be {PROOF_WORDS}",
                self.0.len(),
            )));
        }
        if self.0[0] != PROOF_PREFIX_WORDS as u64 {
            return Err(Error::InvalidProofFormat(format!(
                "proof n_publics is {}, expected to be {PROOF_PREFIX_WORDS}",
                self.0[0],
            )));
        }
        Ok(())
    }
}

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(ZiskProof, reject_trailing_bytes);

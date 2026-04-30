use core::array::from_fn;

use airbender_host::raw::UnrolledProgramProof;
use serde::{Deserialize, Serialize};

/// A proof produced by the host prover that bundles everything needed for
/// verification.
///
/// Wraps `airbender-host`'s [`UnrolledProgramProof`], serialized via bincode legacy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AirbenderProof(pub UnrolledProgramProof);

impl AirbenderProof {
    /// Public values as bytes from the `register_final_values`.
    pub fn public_values(&self) -> [u8; 32] {
        words_to_le_bytes(from_fn(|i| self.0.register_final_values[10 + i].value))
    }
}

pub fn words_to_le_bytes(words: [u32; 8]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes
        .chunks_exact_mut(4)
        .zip(words)
        .for_each(|(bytes, word)| bytes.copy_from_slice(&word.to_le_bytes()));
    bytes
}

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(AirbenderProof);

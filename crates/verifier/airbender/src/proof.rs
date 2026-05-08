use airbender_execution_utils::unrolled::UnrolledProgramProof;
use serde::{Deserialize, Serialize};

/// A proof produced by the host prover that bundles everything needed for
/// verification.
///
/// Wraps [`UnrolledProgramProof`], serialized via bincode legacy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AirbenderProof(pub UnrolledProgramProof);

pub fn words_to_le_bytes(words: [u32; 8]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes
        .chunks_exact_mut(4)
        .zip(words)
        .for_each(|(bytes, word)| bytes.copy_from_slice(&word.to_le_bytes()));
    bytes
}

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(AirbenderProof);

use ere_verifier_core::codec::impl_codec_by_bincode_legacy;
use serde::{Deserialize, Serialize};

/// Verifying key for a specific guest program.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AirbenderProgramVk(pub [u32; 8]);

impl_codec_by_bincode_legacy!(AirbenderProgramVk);

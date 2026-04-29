use airbender_host::UnifiedVk;
use ere_verifier_core::codec::impl_codec_by_bincode_legacy;
use serde::{Deserialize, Serialize};

/// Verifying key that identifies a specific compiled guest program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirbenderProgramVk(pub UnifiedVk);

impl_codec_by_bincode_legacy!(AirbenderProgramVk);

use airbender_execution_utils::{setups::CompiledCircuitsSet, unrolled::UnrolledProgramSetup};
use ere_verifier_core::codec::impl_codec_by_bincode_legacy;
use serde::{Deserialize, Serialize};

/// Unified verification key bundle for recursion.
///
/// Vendored from `airbender_host::vk::UnifiedVk`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedVk {
    pub app_bin_hash: [u8; 32],
    pub unified_setup: UnrolledProgramSetup,
    pub unified_layouts: CompiledCircuitsSet,
}

/// Verifying key that identifies a specific compiled guest program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirbenderProgramVk(pub UnifiedVk);

impl_codec_by_bincode_legacy!(AirbenderProgramVk);

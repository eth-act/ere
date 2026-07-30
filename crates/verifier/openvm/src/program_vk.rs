use openvm_verify_stark_host::vk::VerificationBaseline;
use serde::{Deserialize, Serialize};

/// Verifying key that identifies a specific compiled guest program.
///
/// Wraps the upstream [`VerificationBaseline`] which carries the digests and
/// memory dimensions unique to the compiled guest executable.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpenVMProgramVk(pub VerificationBaseline);

impl OpenVMProgramVk {
    pub fn new(baseline: VerificationBaseline) -> Self {
        Self(baseline)
    }
}

ere_verifier_core::codec::impl_codec_by_bitcode!(OpenVMProgramVk);

use ere_verifier_core::codec::{Decode, Encode};
use openvm_verify_stark_host::vk::VerificationBaseline;
use serde::{Deserialize, Serialize};

/// Verifying key that identifies a specific compiled guest program.
///
/// Wraps the upstream [`VerificationBaseline`] which carries the digests and
/// memory dimensions unique to the compiled guest executable.
///
/// Encoded via `bitcode`. Decoding rejects trailing bytes and non-canonical encodings.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpenVMProgramVk(pub VerificationBaseline);

impl OpenVMProgramVk {
    pub fn new(baseline: VerificationBaseline) -> Self {
        Self(baseline)
    }
}

impl Encode for OpenVMProgramVk {
    type Error = bitcode::Error;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        bitcode::serialize(self)
    }
}

impl Decode for OpenVMProgramVk {
    type Error = bitcode::Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        let value: Self = bitcode::deserialize(slice)?;
        if bitcode::serialize(&value)? != slice {
            return Err(<bitcode::Error as serde::de::Error>::custom(
                "non-canonical encoding",
            ));
        }
        Ok(value)
    }
}

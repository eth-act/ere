use miden_core::utils::{Deserializable, Serializable};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

mod miden_asm;

pub use miden_asm::MidenAsm;

/// Wrapper for [`miden_core::Program`] that implements `serde`.
#[derive(Clone)]
pub struct MidenProgram(pub miden_core::Program);

impl Serialize for MidenProgram {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0.to_bytes())
    }
}

impl<'de> Deserialize<'de> for MidenProgram {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        miden_core::Program::read_from_bytes(&bytes)
            .map(Self)
            .map_err(D::Error::custom)
    }
}

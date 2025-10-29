use miden_core::{
    Program, ProgramInfo,
    utils::{Deserializable, Serializable},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use std::ops::Deref;

mod miden_asm;

pub use miden_asm::MidenAsm;

pub type MidenProgram = MidenSerdeWrapper<Program>;
pub type MidenProgramInfo = MidenSerdeWrapper<ProgramInfo>;

/// Wrapper that implements `serde` for Miden structures.
#[derive(Clone)]
pub struct MidenSerdeWrapper<T>(pub T);

impl<T> Deref for MidenSerdeWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Serializable> Serialize for MidenSerdeWrapper<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0.to_bytes())
    }
}

impl<'de, T: Deserializable> Deserialize<'de> for MidenSerdeWrapper<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        T::read_from_bytes(&bytes)
            .map(Self)
            .map_err(D::Error::custom)
    }
}

use crate::ErezkVM;
use anyhow::{Context, Error};
use serde::Serialize;
use zkvm_interface::{Input, InputItem};

impl ErezkVM {
    pub fn serialize_object(&self, obj: &(impl Serialize + ?Sized)) -> Result<Vec<u8>, Error> {
        match self {
            #[cfg(feature = "jolt")]
            Self::Jolt => unimplemented!(),
            #[cfg(feature = "nexus")]
            Self::Nexus => unimplemented!(),
            #[cfg(feature = "openvm")]
            Self::OpenVM => {
                ere_openvm::serialize_object(obj).with_context(|| "Failed to serialize object")
            }
            #[cfg(feature = "pico")]
            Self::Pico => bincode::serialize(obj).with_context(|| "Failed to serialize object"),
            #[cfg(feature = "risc0")]
            Self::Risc0 => {
                ere_risc0::serialize_object(obj).with_context(|| "Failed to serialize object")
            }
            #[cfg(feature = "sp1")]
            Self::SP1 => bincode::serialize(obj).with_context(|| "Failed to serialize object"),
            #[cfg(feature = "zisk")]
            Self::Zisk => bincode::serialize(obj).with_context(|| "Failed to serialize object"),
        }
    }

    pub fn serialize_inputs(&self, inputs: &Input) -> Result<Vec<u8>, Error> {
        bincode::serialize(
            &inputs
                .iter()
                .map(|input| {
                    Ok(match input {
                        InputItem::Object(obj) => self.serialize_object(&**obj)?,
                        InputItem::Bytes(bytes) => bytes.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?,
        )
        .with_context(|| "Failed to serialize input")
    }
}

pub fn deserialize_inputs(bytes: &[u8]) -> Result<Input, Error> {
    bincode::deserialize::<Vec<Vec<u8>>>(bytes)
        .map(|inputs| Vec::from_iter(inputs.into_iter().map(InputItem::Bytes)).into())
        .with_context(|| "Failed to deserialize input")
}

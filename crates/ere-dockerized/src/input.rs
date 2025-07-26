use crate::ErezkVM;
use anyhow::{Context, Error};
use serde::Serialize;
use zkvm_interface::{Input, InputItem};

impl ErezkVM {
    pub fn serialize_object(&self, obj: &(impl Serialize + ?Sized)) -> Result<Vec<u8>, Error> {
        match self {
            Self::Jolt => unimplemented!(),
            Self::Nexus => unimplemented!(),
            // FIXME: Instead of using `openvm::serde::to_vec`, we use Risc0's
            //        serializer, because OpenVM uses the same one, to avoid the
            //        duplicated extern symbol they export.
            //        It'd be better to have each zkvm provides their
            //        lightweight serde crate.
            Self::OpenVM => risc0_zkvm::serde::to_vec(obj)
                .map(|words| words.into_iter().flat_map(|w| w.to_le_bytes()).collect())
                .with_context(|| "Failed to serialize object"),
            Self::Pico => bincode::serialize(obj).with_context(|| "Failed to serialize object"),
            Self::Risc0 => risc0_zkvm::serde::to_vec(obj)
                .map(|vec| bytemuck::cast_slice(&vec).to_vec())
                .with_context(|| "Failed to serialize object"),
            Self::SP1 => bincode::serialize(obj).with_context(|| "Failed to serialize object"),
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

use crate::{ErezkVM, error::CommonError};
use ere_server::input::{SerializedInput, SerializedInputItem};
use ere_zkvm_interface::{Input, InputItem};
use serde::Serialize;

impl ErezkVM {
    pub fn serialize_object(
        &self,
        obj: &(impl Serialize + ?Sized),
    ) -> Result<Vec<u8>, CommonError> {
        match self {
            // Issue for tracking: https://github.com/eth-act/ere/issues/4.
            Self::Jolt => todo!(),
            Self::Miden => bincode::serialize(obj).map_err(|err| {
                CommonError::serilization(err, "Failed to serialize object with `bincode`")
            }),
            // Issue for tracking: https://github.com/eth-act/ere/issues/63.
            Self::Nexus => todo!(),
            // FIXME: Instead of using `openvm::serde::to_vec`, we use Risc0's
            //        serializer, because OpenVM uses the same one, to avoid the
            //        duplicated extern symbol they export.
            //        It'd be better to have each zkvm provides their
            //        lightweight serde crate.
            //        The issue for tracking https://github.com/eth-act/ere/issues/76.
            Self::OpenVM => risc0_zkvm::serde::to_vec(obj)
                .map(|words| words.into_iter().flat_map(|w| w.to_le_bytes()).collect())
                .map_err(|err| {
                    CommonError::serilization(
                        err,
                        "Failed to serialize object with `risc0_zkvm::serde::to_vec`",
                    )
                }),
            Self::Pico => bincode::serialize(obj).map_err(|err| {
                CommonError::serilization(err, "Failed to serialize object with `bincode`")
            }),
            Self::Risc0 => risc0_zkvm::serde::to_vec(obj)
                .map(|vec| bytemuck::cast_slice(&vec).to_vec())
                .map_err(|err| {
                    CommonError::serilization(
                        err,
                        "Failed to serialize object with `risc0_zkvm::serde::to_vec`",
                    )
                }),
            Self::SP1 => bincode::serialize(obj).map_err(|err| {
                CommonError::serilization(err, "Failed to serialize object with `bincode`")
            }),
            Self::Ziren => bincode::serialize(obj).map_err(|err| {
                CommonError::serilization(err, "Failed to serialize object with `bincode`")
            }),
            Self::Zisk => bincode::serialize(obj).map_err(|err| {
                CommonError::serilization(err, "Failed to serialize object with `bincode`")
            }),
        }
    }

    pub fn serialize_inputs(&self, inputs: &Input) -> Result<SerializedInput, CommonError> {
        inputs
            .iter()
            .map(|input| {
                Ok(match input {
                    InputItem::Object(obj) => {
                        SerializedInputItem::SerializedObject(self.serialize_object(&**obj)?)
                    }
                    InputItem::SerializedObject(bytes) => {
                        SerializedInputItem::SerializedObject(bytes.clone())
                    }
                    InputItem::Bytes(bytes) => SerializedInputItem::Bytes(bytes.clone()),
                })
            })
            .collect::<Result<_, _>>()
            .map(SerializedInput)
    }
}

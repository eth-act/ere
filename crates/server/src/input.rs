use ere_zkvm_interface::{Input, InputItem};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SerializedInput(pub Vec<SerializedInputItem>);

impl From<SerializedInput> for Input {
    fn from(value: SerializedInput) -> Self {
        Self::from(value.0.into_iter().map(Into::into).collect::<Vec<_>>())
    }
}

/// `InputItem` but only `SerializedObject` and `Byte` variants remain.
///
/// The user must serialize the `InputItem::Object` in the way the zkVM expects.
#[derive(Serialize, Deserialize)]
pub enum SerializedInputItem {
    SerializedObject(Vec<u8>),
    Bytes(Vec<u8>),
}

impl From<SerializedInputItem> for InputItem {
    fn from(value: SerializedInputItem) -> Self {
        match value {
            SerializedInputItem::SerializedObject(bytes) => Self::SerializedObject(bytes),
            SerializedInputItem::Bytes(bytes) => Self::Bytes(bytes),
        }
    }
}

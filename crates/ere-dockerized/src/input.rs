use anyhow::{Context, Error};
use serde::Serialize;
use zkvm_interface::{Input, InputItem};

pub fn serialize_object(obj: &(impl Serialize + ?Sized)) -> Result<Vec<u8>, Error> {
    #[cfg(feature = "jolt")]
    unimplemented!();

    #[cfg(feature = "nexus")]
    unimplemented!();

    #[cfg(feature = "openvm")]
    return ere_openvm::serialize_object(obj).with_context(|| "Failed to serialize object");

    #[cfg(feature = "pico")]
    return bincode::serialize(obj).with_context(|| "Failed to serialize object");

    #[cfg(feature = "risc0")]
    return ere_risc0::serialize_object(obj).with_context(|| "Failed to serialize object");

    #[cfg(feature = "sp1")]
    return bincode::serialize(obj).with_context(|| "Failed to serialize object");

    #[cfg(feature = "zisk")]
    return bincode::serialize(obj).with_context(|| "Failed to serialize object");
}

pub fn serialize_inputs(inputs: &Input) -> Result<Vec<u8>, Error> {
    bincode::serialize(
        &inputs
            .iter()
            .map(|input| {
                Ok(match input {
                    InputItem::Object(obj) => serialize_object(&**obj)?,
                    InputItem::Bytes(bytes) => bytes.clone(),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?,
    )
    .with_context(|| "Failed to serialize input")
}

pub fn deserialize_inputs(bytes: &[u8]) -> Result<Input, Error> {
    bincode::deserialize::<Vec<Vec<u8>>>(bytes)
        .map(|inputs| Vec::from_iter(inputs.into_iter().map(InputItem::Bytes)).into())
        .with_context(|| "Failed to deserialize input")
}

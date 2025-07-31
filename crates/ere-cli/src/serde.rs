use anyhow::{Context, Error};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::Path};
use zkvm_interface::{Input, InputItem};

/// Read `Input` from `input_path`.
///
/// `Input` is assumed to be serialized into sequence of bytes, and each bytes
/// in the sequence is serialized in the specific way the zkvm does.
pub fn read_input(input_path: &Path) -> Result<Input, Error> {
    read::<Vec<Vec<u8>>>(input_path, "input")
        .map(|seq| Input::from(Vec::from_iter(seq.into_iter().map(InputItem::Bytes))))
}

/// Serialize `value` with [`bincode`] and write to `path`.
pub fn write<P: Serialize>(path: &Path, value: &P, identifier: &str) -> Result<(), Error> {
    let bytes =
        bincode::serialize(value).with_context(|| format!("Failed to serialize {identifier}"))?;
    fs::write(path, &bytes)
        .with_context(|| format!("Failed to write {identifier} at {}", path.display()))
}

/// Read from `path` and deserialize with [`bincode`].
pub fn read<P: DeserializeOwned>(path: &Path, identifier: &str) -> Result<P, Error> {
    let bytes = fs::read(path)
        .with_context(|| format!("Failed to read {identifier} at {}", path.display()))?;
    bincode::deserialize(&bytes).with_context(|| "Failed to deserialize {identifier}")
}

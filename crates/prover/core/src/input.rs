use bincode::error::{DecodeError, EncodeError};
use serde::{Serialize, de::DeserializeOwned};

/// Input for the prover to execute/prove a guest program.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub stdin: Vec<u8>,
    /// Serialized proofs to be verified in guest program for proof composition.
    pub proofs: Option<Vec<u8>>,
}

impl Input {
    /// Creates a new `Input` with the empty stdin.
    pub fn new() -> Self {
        Self {
            stdin: Vec::new(),
            proofs: None,
        }
    }

    /// Returns a reference to the stdin as a byte slice.
    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }

    /// Deserializes and returns the proofs if present.
    ///
    /// # Returns
    ///
    /// - `None` if no proofs are set
    /// - `Some(Ok(..))` if the proofs were successfully deserialized
    /// - `Some(Err(..))` if deserialization failed
    pub fn proofs<T: DeserializeOwned>(&self) -> Option<Result<Vec<T>, DecodeError>> {
        self.proofs.as_ref().map(|proofs| {
            bincode::serde::decode_from_slice(proofs, bincode::config::legacy())
                .map(|(proofs, _)| proofs)
        })
    }

    /// Sets stdin and returns a new `Input`.
    pub fn with_stdin(mut self, stdin: Vec<u8>) -> Self {
        self.stdin = stdin;
        self
    }

    /// Sets stdin with a LE u32 length prefix and returns a new `Input`.
    ///
    /// The `Platform::read_whole_input` requires stdin to have a LE u32 length
    /// prefix for efficiency reason.
    pub fn with_prefixed_stdin(mut self, stdin: Vec<u8>) -> Self {
        self.stdin = Vec::with_capacity(4 + stdin.len());
        self.stdin.extend((stdin.len() as u32).to_le_bytes());
        self.stdin.extend(stdin);
        self
    }

    /// Serializes the given proofs and returns a new `Input` with them set.
    ///
    /// Consumes `self` and returns an error if serialization fails.
    pub fn with_proofs<T: Serialize>(mut self, proofs: &[T]) -> Result<Self, EncodeError> {
        self.proofs = Some(bincode::serde::encode_to_vec(
            proofs,
            bincode::config::legacy(),
        )?);
        Ok(self)
    }

    /// Sets serialized proofs and returns a new `Input`.
    ///
    /// The proofs must be serialized using [`bincode::serde`] with
    /// [`bincode::config::legacy`] configuration.
    pub fn with_serialized_proofs(mut self, proofs: Vec<u8>) -> Self {
        self.proofs = Some(proofs);
        self
    }
}

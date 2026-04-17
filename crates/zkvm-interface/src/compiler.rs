use serde::{Deserialize, Serialize};
use std::{error::Error, ops::Deref, path::Path};

/// ELF binary of a compiled guest program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Elf(pub Vec<u8>);

impl Deref for Elf {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for Elf {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for Elf {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

/// Compiler trait for compiling guest programs into an [`Elf`] binary.
pub trait Compiler {
    type Error: Error + Send + Sync + 'static;

    /// Compiles the program and returns the [`Elf`]
    ///
    /// # Arguments
    /// * `guest_directory` - The path to the guest program directory
    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error>;
}

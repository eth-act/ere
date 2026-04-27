use core::ops::Deref;

use serde::{Deserialize, Serialize};

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

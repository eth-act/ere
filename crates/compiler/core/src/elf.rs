use core::{fmt, ops::Deref};

use serde::{Deserialize, Serialize};

/// ELF binary of a compiled guest program.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Elf(pub Vec<u8>);

impl fmt::Debug for Elf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Elf").field("len", &self.0.len()).finish()
    }
}

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

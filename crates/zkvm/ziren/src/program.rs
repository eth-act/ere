use serde::{Deserialize, Serialize};

/// Ziren program that contains ELF of compiled guest.
#[derive(Clone, Serialize, Deserialize)]
pub struct ZirenProgram {
    pub(crate) elf: Vec<u8>,
}

impl ZirenProgram {
    pub fn elf(&self) -> &[u8] {
        &self.elf
    }
}

use serde::{Deserialize, Serialize};

/// Pico program that contains ELF of compiled guest.
#[derive(Clone, Serialize, Deserialize)]
pub struct PicoProgram {
    pub(crate) elf: Vec<u8>,
}

impl PicoProgram {
    pub fn elf(&self) -> &[u8] {
        &self.elf
    }
}

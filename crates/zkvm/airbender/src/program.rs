use serde::{Deserialize, Serialize};

/// Airbender program that contains binary format of compiled guest.
#[derive(Clone, Serialize, Deserialize)]
pub struct AirbenderProgram {
    pub(crate) bin: Vec<u8>,
    pub(crate) elf: Vec<u8>,
}

impl AirbenderProgram {
    pub fn bin(&self) -> &[u8] {
        &self.bin
    }

    pub fn elf(&self) -> &[u8] {
        &self.elf
    }
}

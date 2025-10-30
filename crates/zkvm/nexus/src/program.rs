use serde::{Deserialize, Serialize};

/// Nexus program that contains ELF of compiled guest.
#[derive(Clone, Serialize, Deserialize)]
pub struct NexusProgram {
    pub(crate) elf: Vec<u8>,
}

impl NexusProgram {
    pub fn elf(&self) -> &[u8] {
        &self.elf
    }
}

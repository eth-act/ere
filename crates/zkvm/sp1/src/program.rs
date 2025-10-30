use serde::{Deserialize, Serialize};

/// SP1 program that contains ELF of compiled guest.
#[derive(Clone, Serialize, Deserialize)]
pub struct SP1Program {
    pub(crate) elf: Vec<u8>,
}

impl SP1Program {
    pub fn elf(&self) -> &[u8] {
        &self.elf
    }
}

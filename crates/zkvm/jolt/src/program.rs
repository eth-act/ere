use serde::{Deserialize, Serialize};

/// Jolt program that contains ELF of compiled guest.
#[derive(Clone, Serialize, Deserialize)]
pub struct JoltProgram {
    pub(crate) elf: Vec<u8>,
}

impl JoltProgram {
    pub fn elf(&self) -> &[u8] {
        &self.elf
    }
}

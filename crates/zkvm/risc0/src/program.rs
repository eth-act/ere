use risc0_zkp::core::digest::Digest;
use serde::{Deserialize, Serialize};

/// Risc0 program that contains ELF of compiled guest and image ID.
#[derive(Clone, Serialize, Deserialize)]
pub struct Risc0Program {
    pub(crate) elf: Vec<u8>,
    pub(crate) image_id: Digest,
}

impl Risc0Program {
    pub fn elf(&self) -> &[u8] {
        &self.elf
    }

    pub fn image_id(&self) -> &Digest {
        &self.image_id
    }
}

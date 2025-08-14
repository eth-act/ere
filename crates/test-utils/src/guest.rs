use alloc::vec::Vec;
use core::iter;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct BasicStruct {
    pub a: u8,
    pub b: u16,
    pub c: u32,
    pub d: u64,
    pub e: Vec<u8>,
}

impl BasicStruct {
    /// Performs some computation (Xoring all fields as bytes into `[u8; 32]`).
    pub fn output(&self) -> [u8; 32] {
        let mut output = [0; 32];
        iter::empty()
            .chain(self.a.to_le_bytes())
            .chain(self.b.to_le_bytes())
            .chain(self.c.to_le_bytes())
            .chain(self.d.to_le_bytes())
            .chain(self.e.iter().copied())
            .enumerate()
            .for_each(|(idx, byte)| output[idx % output.len()] ^= byte);
        output
    }
}

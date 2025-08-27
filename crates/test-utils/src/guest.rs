use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub const BASIC_PROGRAM_BYTES_LENGTH: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicStruct {
    pub a: u8,
    pub b: u16,
    pub c: u32,
    pub d: u64,
    pub e: Vec<u8>,
}

impl BasicStruct {
    #[cfg(feature = "host")]
    pub fn random(mut rng: impl rand::Rng) -> Self {
        let n = rng.random_range(16..32);
        BasicStruct {
            a: rng.random(),
            b: rng.random(),
            c: rng.random(),
            d: rng.random(),
            e: rng.random_iter().take(n).collect(),
        }
    }

    /// Performs some computation (Wrapping add all fields by 1).
    pub fn output(&self) -> Self {
        Self {
            a: self.a.wrapping_add(1),
            b: self.b.wrapping_add(1),
            c: self.c.wrapping_add(1),
            d: self.d.wrapping_add(1),
            e: self.e.iter().map(|byte| byte.wrapping_add(1)).collect(),
        }
    }
}

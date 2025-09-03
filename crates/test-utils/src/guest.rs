use alloc::vec::Vec;
use core::iter;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub struct BasicProgramCore;

impl BasicProgramCore {
    pub const BYTES_LENGTH: usize = 32;

    pub fn outputs(inputs: (Vec<u8>, BasicStruct)) -> (Vec<u8>, BasicStruct) {
        let (bytes, basic_struct) = inputs;
        (bytes.iter().rev().copied().collect(), basic_struct.output())
    }

    pub fn sha256_outputs(outputs: (Vec<u8>, BasicStruct)) -> [u8; 32] {
        let (rev_bytes, basic_struct) = outputs;
        Sha256::digest(
            iter::empty()
                .chain(rev_bytes)
                .chain(bincode::serialize(&basic_struct).unwrap())
                .collect::<Vec<_>>(),
        )
        .into()
    }
}

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

use core::mem::ManuallyDrop;

use proofman_util::VadcopFinalProof;
use serde::{Deserialize, Serialize};

use crate::{Error, ZiskProgramVk};

/// Size of the public values in bytes (64 slots * 4 bytes each).
pub const PUBLIC_VALUES_SIZE: usize = 256;

/// Zisk VadcopFinalProof with strong type of `program_vk` and `public_values`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZiskProof {
    pub proof: Vec<u64>,
    pub program_vk: ZiskProgramVk,
    #[serde(with = "serde_big_array::BigArray")]
    pub public_values: [u8; PUBLIC_VALUES_SIZE],
}

impl ZiskProof {
    /// Returns the program verifying key embedded in this proof.
    pub fn program_vk(&self) -> ZiskProgramVk {
        self.program_vk
    }

    /// Converts this proof into the `VadcopFinalProof` format expected by the proofman verifier.
    pub fn vadcop_final_proof(&self) -> Result<VadcopFinalProof, Error> {
        let proof = cast_bytes(self.proof.clone());

        let public_values = {
            let program_vk = self.program_vk.0;
            let public_values_words = self
                .public_values
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()) as u64);
            cast_bytes(program_vk.into_iter().chain(public_values_words).collect())
        };

        Ok(VadcopFinalProof {
            proof,
            public_values,
            compressed: false,
        })
    }
}

/// Converts a `Vec<u64>` into a `Vec<u8>` preserving the u64-aligned allocation.
fn cast_bytes(data: Vec<u64>) -> Vec<u8> {
    let mut data = ManuallyDrop::new(data);
    let ptr = data.as_mut_ptr().cast::<u8>();
    let len = data.len() * size_of::<u64>();
    let cap = data.capacity() * size_of::<u64>();
    // SAFETY: `ptr` came from a `Vec<u64>` allocation.
    unsafe { Vec::from_raw_parts(ptr, len, cap) }
}

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(ZiskProof);

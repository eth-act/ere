use alloc::vec::Vec;
use core::{array::from_fn, convert::Infallible};

use ere_verifier_core::codec::{Decode, Encode};
use risc0_zkvm::Digest;
use serde::{Deserialize, Serialize};

use crate::Error;

const PROGRAM_VK_SIZE: usize = 32;

/// Verifying key that identifies a specific compiled guest program.
///
/// Produced during setup and consumed by [`Risc0Verifier`] together with a [`Risc0Proof`] to
/// authenticate that the proof was generated from the same program. Wraps a 32-byte `Digest`
/// produced by `risc0_binfmt::compute_image_id`; encoded as 8 little-endian u32 words.
///
/// [`Risc0Verifier`]: crate::Risc0Verifier
/// [`Risc0Proof`]: crate::Risc0Proof
#[derive(Copy, Clone, Eq, Ord, PartialOrd, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Risc0ProgramVk(pub Digest);

impl Encode for Risc0ProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        let words = self.0.as_words();
        Ok(words.iter().flat_map(|word| word.to_le_bytes()).collect())
    }
}

impl Decode for Risc0ProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let words = from_fn(|i| u32::from_le_bytes(from_fn(|j| slice[4 * i + j])));
        Ok(Self(Digest::from(words)))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(Risc0ProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(Risc0ProgramVk);

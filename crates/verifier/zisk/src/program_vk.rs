use core::{array::from_fn, convert::Infallible};

use ere_verifier_core::codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::Error;

const PROGRAM_VK_SIZE: usize = 32;

/// Verifying key that identifies a specific compiled guest program.
///
/// Produced during setup and consumed by [`ZiskVerifier`] together with a
/// [`ZiskProof`] to authenticate that the proof was generated from the same
/// program. The Merkle root of the ROM trace, held as 4 little-endian u64
/// limbs (32 bytes total).
///
/// [`ZiskVerifier`]: crate::ZiskVerifier
/// [`ZiskProof`]: crate::ZiskProof
#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ZiskProgramVk(pub [u64; 4]);

impl Encode for ZiskProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.0.iter().flat_map(|word| word.to_le_bytes()).collect())
    }
}

impl Decode for ZiskProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let words = from_fn(|i| u64::from_le_bytes(from_fn(|j| slice[8 * i + j])));
        Ok(Self(words))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(ZiskProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(ZiskProgramVk);

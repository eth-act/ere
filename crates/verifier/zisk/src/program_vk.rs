use core::{array::from_fn, convert::Infallible};

use ere_verifier_core::codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{Error, proof::PROGRAM_VK_WORDS};

const PROGRAM_VK_BYTES: usize = PROGRAM_VK_WORDS * 8;

/// Goldilocks field order (`2^64 - 2^32 + 1`); inner u64s must be strictly less.
const GOLDILOCKS_ORDER: u64 = 0xFFFF_FFFF_0000_0001;

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

impl TryFrom<&[u64]> for ZiskProgramVk {
    type Error = Error;

    fn try_from(value: &[u64]) -> Result<Self, Self::Error> {
        if value.len() != 4 {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_BYTES,
                got: value.len() * 8,
            });
        }
        Ok(Self(value.try_into().unwrap()))
    }
}

impl Encode for ZiskProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.0.iter().flat_map(|word| word.to_le_bytes()).collect())
    }
}

impl Decode for ZiskProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_BYTES {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_BYTES,
                got: slice.len(),
            });
        }
        let words = from_fn(|i| u64::from_le_bytes(from_fn(|j| slice[8 * i + j])));
        if words.iter().any(|word| *word >= GOLDILOCKS_ORDER) {
            return Err(Error::NonCanonicalProgramVk);
        }
        Ok(Self(words))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(ZiskProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(ZiskProgramVk);

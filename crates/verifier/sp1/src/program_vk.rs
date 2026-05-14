use core::{array::from_fn, convert::Infallible};

use ere_verifier_core::codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sp1_hypercube::{DIGEST_SIZE, PrimeField32};
use sp1_primitives::SP1Field;

use crate::Error;

const PROGRAM_VK_SIZE: usize = 32;

/// Verifying key that identifies a specific compiled guest program.
///
/// Wraps [`sp1_hypercube::HashableKey::hash_u32`] output of an [`sp1_hypercube::SP1VerifyingKey`].
#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SP1ProgramVk(pub [SP1Field; DIGEST_SIZE]);

impl Encode for SP1ProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        let words = self.0.map(|word| word.as_canonical_u32());
        Ok(words.iter().flat_map(|word| word.to_le_bytes()).collect())
    }
}

impl Decode for SP1ProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let words = from_fn(|i| from_u32(u32::from_le_bytes(from_fn(|j| slice[4 * i + j]))));
        Ok(Self(words))
    }
}

fn from_u32<F: PrimeField32>(word: u32) -> F {
    F::from_canonical_u32(word)
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(SP1ProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(SP1ProgramVk);

use core::{array::from_fn, convert::Infallible};

use ere_verifier_core::codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sp1_hypercube::{DIGEST_SIZE, PrimeField32};
use sp1_primitives::SP1Field;

use crate::Error;

const PROGRAM_VK_SIZE: usize = 32;

/// Number of limbs the packed integer is manipulated in.
const PROGRAM_VK_LIMBS: usize = PROGRAM_VK_SIZE / 8;

/// Width of the slot each koalabear element occupies in the packed integer.
const WORD_BITS: u32 = 31;

/// Verifying key that identifies a specific compiled guest program.
///
/// Wraps poseidon2 digest of an [`sp1_hypercube::SP1VerifyingKey`], produced by
/// [`sp1_hypercube::HashableKey::hash_koalabear`].
///
/// Encodes to the 32-byte form as [`sp1_hypercube::HashableKey::bytes32`],
/// which packs the elements as base-`2^31` digits of a big endian integer with
/// the first element most significant.
#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SP1ProgramVk(pub [SP1Field; DIGEST_SIZE]);

impl Encode for SP1ProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        let mut limbs = [0u64; PROGRAM_VK_LIMBS];
        for word in self.0 {
            for i in (1..PROGRAM_VK_LIMBS).rev() {
                limbs[i] = (limbs[i] << WORD_BITS) | (limbs[i - 1] >> (u64::BITS - WORD_BITS));
            }
            limbs[0] = (limbs[0] << WORD_BITS) | u64::from(word.as_canonical_u32());
        }
        Ok(limbs
            .iter()
            .rev()
            .flat_map(|limb| limb.to_be_bytes())
            .collect())
    }
}

impl Decode for SP1ProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        const WORD_MASK: u64 = (1 << WORD_BITS) - 1;

        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let mut limbs: [u64; PROGRAM_VK_LIMBS] = from_fn(|i| {
            let offset = PROGRAM_VK_SIZE - 8 * (i + 1);
            u64::from_be_bytes(from_fn(|j| slice[offset + j]))
        });
        let mut words = [0u32; DIGEST_SIZE];
        for word in words.iter_mut().rev() {
            *word = (limbs[0] & WORD_MASK) as u32;
            for i in 0..PROGRAM_VK_LIMBS - 1 {
                limbs[i] = (limbs[i] >> WORD_BITS) | (limbs[i + 1] << (u64::BITS - WORD_BITS));
            }
            limbs[PROGRAM_VK_LIMBS - 1] >>= WORD_BITS;
        }
        if limbs != [0; PROGRAM_VK_LIMBS] || words.iter().any(|word| *word >= SP1Field::ORDER_U32) {
            return Err(Error::NonCanonicalProgramVk);
        }
        Ok(Self(words.map(from_canonical_u32)))
    }
}

fn from_canonical_u32<F: PrimeField32>(word: u32) -> F {
    F::from_canonical_u32(word)
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(SP1ProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(SP1ProgramVk);

#[cfg(test)]
mod tests {
    use super::*;

    /// A digest whose leading element is below `2^23` packs into an integer
    /// narrower than 248 bits, which must still encode as a full 32 bytes.
    #[test]
    fn round_trip_small_leading_element() {
        for leading in [0, 1, (1 << 23) - 1, 1 << 23, SP1Field::ORDER_U32 - 1] {
            let words: [u32; DIGEST_SIZE] = from_fn(|i| if i == 0 { leading } else { 0x1234_5678 });
            let program_vk = SP1ProgramVk(words.map(from_canonical_u32));

            let encoded = program_vk.encode_to_vec().unwrap();
            assert_eq!(encoded.len(), PROGRAM_VK_SIZE);
            assert_eq!(
                SP1ProgramVk::decode_from_slice(&encoded).unwrap(),
                program_vk
            );
        }
    }
}

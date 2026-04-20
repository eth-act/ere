use core::{array::from_fn, convert::Infallible};

use ere_verifier_core::codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::Error;

const PROGRAM_VK_SIZE: usize = 32;

/// Verification key hash chain.
///
/// For recursive verifier program, it exposes the chaining hash of verification
/// keys of programs that it verifies, which is computed as
/// `blake(blake(blake(0 || base_vk)|| verifier_0_vk) || verifier_1_vk)...`.
///
/// For a base program, the VK is computed as `blake(PC || setup_caps)`, where
/// `PC` is the program counter value at the end of execution, and `setup_caps`
/// is the merkle tree caps derived from the program.
#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct AirbenderProgramVk(pub [u32; 8]);

impl Encode for AirbenderProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.0.iter().flat_map(|word| word.to_le_bytes()).collect())
    }
}

impl Decode for AirbenderProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let words = from_fn(|i| u32::from_le_bytes(from_fn(|j| slice[4 * i + j])));
        Ok(Self(words))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(AirbenderProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(AirbenderProgramVk);

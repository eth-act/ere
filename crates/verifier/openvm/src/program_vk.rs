use std::{array::from_fn, convert::Infallible};

use ere_verifier_core::codec::{Decode, Encode};
use openvm_stark_sdk::{
    config::baby_bear_poseidon2::{DIGEST_SIZE, F},
    openvm_stark_backend::p3_field::{PrimeCharacteristicRing, PrimeField32},
};
use openvm_verify_stark_host::vk::VerificationBaseline;
use serde::{Deserialize, Serialize};

use crate::{Error, verifier::vk::BASELINE};

const PROGRAM_VK_SIZE: usize = 4 * DIGEST_SIZE;

/// Verifying key of a guest program, the `app_exe_commit` of its [`VerificationBaseline`].
///
/// Encodes each element as a canonical `u32` in little endian.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpenVMProgramVk(pub [F; DIGEST_SIZE]);

impl OpenVMProgramVk {
    pub fn new(app_exe_commit: [F; DIGEST_SIZE]) -> Self {
        Self(app_exe_commit)
    }

    /// Returns the full baseline under the config of `ere-prover-openvm`.
    pub fn baseline(&self) -> VerificationBaseline {
        VerificationBaseline {
            app_exe_commit: self.0,
            ..BASELINE.clone()
        }
    }
}

impl Encode for OpenVMProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self
            .0
            .iter()
            .flat_map(|element| element.as_canonical_u32().to_le_bytes())
            .collect())
    }
}

impl Decode for OpenVMProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let words: [u32; DIGEST_SIZE] =
            from_fn(|i| u32::from_le_bytes(from_fn(|j| slice[4 * i + j])));
        if words.iter().any(|word| *word >= F::ORDER_U32) {
            return Err(Error::NonCanonicalProgramVk);
        }
        Ok(Self(words.map(F::from_u32)))
    }
}

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
        Ok(u32s_to_bytes(
            &self.0.map(|element| element.as_canonical_u32()),
        ))
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
        let words = bytes_to_u32s(slice);
        if words.iter().any(|word| *word >= F::ORDER_U32) {
            return Err(Error::NonCanonicalProgramVk);
        }
        Ok(Self(from_fn(|i| F::from_u32(words[i]))))
    }
}

/// Encodes `limbs` as `u32` in little endian.
fn u32s_to_bytes(limbs: &[u32]) -> Vec<u8> {
    limbs.iter().flat_map(|limb| limb.to_le_bytes()).collect()
}

/// Inverse of [`u32s_to_bytes`].
fn bytes_to_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect()
}

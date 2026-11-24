use std::convert::Infallible;

use ere_verifier_core::codec::{Decode, Encode};
use pico_verifier::{VmVk, vm_vk::VM_VK_LEN};

use crate::Error;

const PROGRAM_VK_SIZE: usize = 4 * VM_VK_LEN;

/// Verifying key of a guest program, its `vm_vk` in the VK bundle of `pico-verifier`.
///
/// Encodes each limb as a canonical `u32` in little endian.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicoProgramVk(pub VmVk);

impl PicoProgramVk {
    pub fn new(vm_vk: VmVk) -> Self {
        Self(vm_vk)
    }
}

impl Encode for PicoProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(u32s_to_bytes(&self.0.to_u32s()))
    }
}

impl Decode for PicoProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let limbs = bytes_to_u32s(slice);
        let vm_vk = VmVk::from_u32s(&limbs).unwrap();
        if vm_vk.to_u32s() != limbs {
            return Err(Error::NonCanonicalProgramVk);
        }
        Ok(Self(vm_vk))
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

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(PicoProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(PicoProgramVk);

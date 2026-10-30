use ere_verifier_core::codec::{Decode, Encode};
use pico_verifier::bundle::{DecodedVkBundle, decode_vk_bundle_v2, encode_vk_bundle_v2};

use crate::Error;

/// Verifying key that identifies a specific compiled guest program.
///
/// The v2 VK bundle of `pico-verifier`: the program's exe identity, the pinned
/// whitelist root of the recursion verifying keys, and the Merkle memberships of
/// the keys a root proof can carry.
///
/// Encoded with the bundle codec of `pico-verifier`. [`PicoVerifier::new`] checks
/// the bundle against the whitelist root shipped with Pico.
///
/// [`PicoVerifier::new`]: crate::PicoVerifier::new
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PicoProgramVk(pub DecodedVkBundle);

impl Encode for PicoProgramVk {
    type Error = Error;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        encode_vk_bundle_v2(&self.0.vm_vk, &self.0.vk_root, &self.0.members)
            .map_err(Error::EncodeProgramVk)
    }
}

impl Decode for PicoProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        decode_vk_bundle_v2(slice)
            .map(Self)
            .map_err(Error::DecodeProgramVk)
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(PicoProgramVk);
ere_verifier_core::codec::impl_try_into_bytes_by_encode!(PicoProgramVk);

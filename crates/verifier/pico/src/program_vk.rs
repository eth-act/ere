use core::{convert::Infallible, fmt};
use std::sync::Arc;

use ere_verifier_core::codec::{Decode, Encode};
use pico_verifier::{
    MleApexVerifier,
    bundle::{apex_from_bundle, decode_vk_bundle_v2},
    shipped_vk_root,
};

use crate::Error;

/// Verifying key that identifies a specific compiled guest program.
///
/// The v2 VK bundle of `pico-verifier`: the program's exe identity, the pinned
/// whitelist root of the recursion verifying keys, and the Merkle memberships of
/// the keys a root proof can carry.
///
/// Encodes to the bundle bytes. Decoding refuses a bundle whose whitelist root
/// is not the one shipped with Pico, checks every membership against it, and
/// keeps the verifier built from the bundle. Equality compares the bundle bytes; another
/// compression of the same bundle decodes to the same key.
#[derive(Clone)]
pub struct PicoProgramVk {
    bundle: Vec<u8>,
    apex: Arc<MleApexVerifier>,
}

impl PicoProgramVk {
    pub fn new(bundle: Vec<u8>) -> Result<Self, Error> {
        let apex = decode_vk_bundle_v2(&bundle)
            .and_then(|decoded| apex_from_bundle(&decoded, shipped_vk_root()))
            .map_err(Error::DecodeProgramVk)?;
        Ok(Self {
            bundle,
            apex: Arc::new(apex),
        })
    }

    /// The VK bundle bytes.
    pub fn bundle(&self) -> &[u8] {
        &self.bundle
    }

    pub(crate) fn apex(&self) -> &MleApexVerifier {
        &self.apex
    }
}

impl fmt::Debug for PicoProgramVk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PicoProgramVk")
            .field("bundle_len", &self.bundle.len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for PicoProgramVk {
    fn eq(&self, other: &Self) -> bool {
        self.bundle == other.bundle
    }
}

impl Eq for PicoProgramVk {}

impl Encode for PicoProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.bundle.clone())
    }
}

impl Decode for PicoProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        Self::new(slice.to_vec())
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(PicoProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(PicoProgramVk);

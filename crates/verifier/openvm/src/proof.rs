use std::io::{self, Cursor};

use ere_verifier_core::codec::{Decode, Encode};
use openvm_stark_sdk::{
    config::baby_bear_poseidon2::DIGEST_SIZE, openvm_stark_backend::codec as stark_backend_codec,
};
use openvm_verify_stark_host::VmStarkProof;

use crate::{Error, verifier::vk::AGG_VK};

/// Offset of the number of AIRs in an encoded proof, which follows the `u32` codec version and the
/// common main commit of `DIGEST_SIZE` field elements of 4 bytes each.
const NUM_AIRS_OFFSET: usize = 4 + 4 * DIGEST_SIZE;

/// Upper bound on the length of an encoded proof, above the largest proof that the shape limits of
/// the aggregation key permit, which measures about 2.4 MB.
const MAX_PROOF_BYTES: usize = 3 * 1024 * 1024;

/// Proof produced by the OpenVM host prover, wrapping the upstream [`VmStarkProof`].
///
/// Encoded via the [`openvm_stark_sdk::openvm_stark_backend::codec`]. Decoding rejects an input
/// longer than `MAX_PROOF_BYTES`, a number of AIRs other than the one of the aggregation
/// key, and trailing bytes.
#[derive(Clone, Debug)]
pub struct OpenVMProof(pub VmStarkProof);

impl OpenVMProof {
    pub fn new(inner: VmStarkProof) -> Self {
        Self(inner)
    }
}

impl Encode for OpenVMProof {
    type Error = Error;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(stark_backend_codec::Encode::encode_to_vec(&self.0)?)
    }
}

impl Decode for OpenVMProof {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() > MAX_PROOF_BYTES {
            return Err(Error::DecodeLimitExceeded {
                limit: MAX_PROOF_BYTES,
                got: slice.len(),
            });
        }
        // The upstream decode allocates 32 bytes for each AIR that one bit of the input declares,
        // so the number of AIRs must match the aggregation key before the decode. This check can go
        // when the decode of the inner proof uses `Proof::decode_using_vk` of stark-backend v2.1.0,
        // which bounds that count by the key.
        if let Some(num_airs) = slice.get(NUM_AIRS_OFFSET..NUM_AIRS_OFFSET + 4) {
            let num_airs = u32::from_le_bytes(num_airs.try_into().unwrap()) as usize;
            if num_airs != AGG_VK.inner.per_air.len() {
                return Err(Error::InvalidNumAirs {
                    expected: AGG_VK.inner.per_air.len(),
                    got: num_airs,
                });
            }
        }
        let mut cursor = Cursor::new(slice);
        let proof = stark_backend_codec::Decode::decode(&mut cursor)?;
        if (cursor.position() as usize) != slice.len() {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "trailing bytes after decoded value",
            ))?;
        }
        Ok(Self(proof))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(OpenVMProof);
ere_verifier_core::codec::impl_try_into_bytes_by_encode!(OpenVMProof);

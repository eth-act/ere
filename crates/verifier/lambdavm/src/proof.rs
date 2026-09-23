use ere_verifier_core::codec::{Decode, Encode, MAX_DECODE_BYTES};
use lambda_vm_prover::VmProof;
use rkyv::{rancor, util::AlignedVec};

use crate::Error;

/// Alignment of the buffer the archived proof is validated in, which is the alignment
/// `lambda-vm-prover` requires for its archives.
const ARCHIVE_ALIGNMENT: usize = 16;

/// Proof produced by the LambdaVM host prover, wrapping the upstream [`VmProof`].
///
/// Encoded via `rkyv`, the same as `lambda-vm-cli prove` writes a proof. Decoding validates the
/// archive, and rejects inputs longer than [`MAX_DECODE_BYTES`].
#[derive(Clone, Debug)]
pub struct LambdaVMProof(pub VmProof);

impl Encode for LambdaVMProof {
    type Error = Error;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(rkyv::to_bytes::<rancor::Error>(&self.0)?.to_vec())
    }
}

impl Decode for LambdaVMProof {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() > MAX_DECODE_BYTES {
            return Err(Error::DecodeLimitExceeded {
                limit: MAX_DECODE_BYTES,
                got: slice.len(),
            });
        }
        // The archive must be aligned, so copy it out of the possibly unaligned input.
        let mut aligned = AlignedVec::<ARCHIVE_ALIGNMENT>::with_capacity(slice.len());
        aligned.extend_from_slice(slice);
        Ok(Self(rkyv::from_bytes::<VmProof, rancor::Error>(&aligned)?))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(LambdaVMProof);
ere_verifier_core::codec::impl_try_into_bytes_by_encode!(LambdaVMProof);

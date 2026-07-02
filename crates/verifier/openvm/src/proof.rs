use std::io::{self, Cursor};

use ere_verifier_core::codec::{Decode, Encode};
use openvm_stark_sdk::openvm_stark_backend::codec as stark_backend_codec;
use openvm_verify_stark_host::VmStarkProof;

use crate::Error;

/// Proof produced by the OpenVM host prover, wrapping the upstream [`VmStarkProof`].
///
/// Encoded via the [`openvm_stark_sdk::openvm_stark_backend::codec`].
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

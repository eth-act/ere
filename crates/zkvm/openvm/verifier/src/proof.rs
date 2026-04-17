use crate::Error;
use ere_verifier_core::codec::{Decode, Encode};
use openvm_continuations::verifier::internal::types::VmStarkProof;
use openvm_sdk::SC;

/// A proof produced by the host prover that bundles everything needed for
/// verification.
///
/// Wraps a `VmStarkProof<SC>`; serialized via `openvm_sdk::codec`.
#[derive(Clone)]
pub struct OpenVMProof(pub VmStarkProof<SC>);

impl Encode for OpenVMProof {
    type Error = Error;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Error> {
        openvm_sdk::codec::Encode::encode_to_vec(&self.0).map_err(Error::Encode)
    }
}

impl Decode for OpenVMProof {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Error> {
        openvm_sdk::codec::Decode::decode(&mut &*slice)
            .map(Self)
            .map_err(Error::Decode)
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(OpenVMProof);
ere_verifier_core::codec::impl_try_into_bytes_by_encode!(OpenVMProof);

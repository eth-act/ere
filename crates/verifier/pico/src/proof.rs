use bincode::{
    config::legacy,
    error::{DecodeError, EncodeError},
};
use ere_verifier_core::codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Upper bound on the bytes that the decode of a proof reads, above the compress proof envelopes of
/// about 270 KB that Pico writes, with room for the committed bytes.
const MAX_PROOF_BYTES: usize = 300 * 1024;

/// Proof produced by the Pico host prover: a whitelist-mode compress proof, the
/// final proof Pico hands out, as a v4 envelope of `pico-verifier`, and the bytes
/// the guest committed, which the proof binds by their SHA-256 digest. The verifier
/// accepts no other kind of proof.
///
/// Encoded via bincode legacy, rejecting trailing bytes. Decoding does not parse
/// the envelope, the verifier does.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PicoProof {
    pub envelope: Vec<u8>,
    pub committed: Vec<u8>,
}

impl PicoProof {
    pub fn new(envelope: Vec<u8>, committed: Vec<u8>) -> Self {
        Self {
            envelope,
            committed,
        }
    }
}

impl Encode for PicoProof {
    type Error = EncodeError;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        bincode::serde::encode_to_vec(self, legacy())
    }
}

impl Decode for PicoProof {
    type Error = DecodeError;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        let (proof, consumed) =
            bincode::serde::decode_from_slice(slice, legacy().with_limit::<MAX_PROOF_BYTES>())?;
        if consumed != slice.len() {
            return Err(DecodeError::Other("trailing bytes after decoded value"));
        }
        Ok(proof)
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(PicoProof);
ere_verifier_core::codec::impl_try_into_bytes_by_encode!(PicoProof);

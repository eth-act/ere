use serde::{Deserialize, Serialize};

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

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(PicoProof, reject_trailing_bytes);
ere_verifier_core::codec::impl_try_from_bytes_by_decode!(PicoProof);
ere_verifier_core::codec::impl_try_into_bytes_by_encode!(PicoProof);

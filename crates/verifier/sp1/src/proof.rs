use bincode::{
    config::legacy,
    error::{DecodeError, EncodeError},
};
use ere_verifier_core::codec::{Decode, Encode, MAX_DECODE_BYTES};
use serde::{Deserialize, Serialize};
use sp1_verifier::ProofFromNetwork;

/// A proof produced by the host prover that bundles everything needed for verification.
///
/// Only the [`Compressed`] variant is accepted by [`SP1Verifier::verify`], any other variant
/// returns [`Error::UnexpectedProofKind`]. Serialized via bincode legacy.
///
/// Decoding rejects trailing bytes and any input that differs from the encoding of the decoded
/// proof.
///
/// [`Compressed`]: sp1_verifier::SP1Proof::Compressed
/// [`SP1Verifier::verify`]: crate::zkVMVerifier::verify
/// [`Error::UnexpectedProofKind`]: crate::Error::UnexpectedProofKind
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SP1Proof(pub ProofFromNetwork);

impl Encode for SP1Proof {
    type Error = EncodeError;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        bincode::serde::encode_to_vec(self, legacy())
    }
}

impl Decode for SP1Proof {
    type Error = DecodeError;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        let (proof, consumed): (Self, _) =
            bincode::serde::decode_from_slice(slice, legacy().with_limit::<MAX_DECODE_BYTES>())?;
        if consumed != slice.len() {
            return Err(DecodeError::Other("trailing bytes after decoded value"));
        }
        if !proof.encode_to_vec().is_ok_and(|encoded| encoded == slice) {
            return Err(DecodeError::Other("non-canonical encoding"));
        }
        Ok(proof)
    }
}

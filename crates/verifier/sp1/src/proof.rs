use serde::{Deserialize, Serialize};
use sp1_verifier::ProofFromNetwork;

/// A proof produced by the host prover that bundles everything needed for verification.
///
/// Only the [`Compressed`] variant is accepted by [`SP1Verifier::verify`], any other variant
/// returns [`Error::UnexpectedProofKind`]. Serialized via bincode legacy.
///
/// [`Compressed`]: sp1_verifier::SP1Proof::Compressed
/// [`SP1Verifier::verify`]: crate::SP1Verifier::verify
/// [`Error::UnexpectedProofKind`]: crate::Error::UnexpectedProofKind
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SP1Proof(pub ProofFromNetwork);

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(SP1Proof, reject_trailing_bytes);

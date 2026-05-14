use serde::{Deserialize, Serialize};
use sp1_verifier::ProofFromNetwork;

/// A proof produced by the host prover that bundles everything needed for
/// verification.
///
/// Wraps `SP1ProofWithPublicValues`; verifiable only when the inner
/// `sp1_sdk::SP1Proof` is `Compressed`. Serialized via bincode legacy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SP1Proof(pub ProofFromNetwork);

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(SP1Proof, reject_trailing_bytes);

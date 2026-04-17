use risc0_zkvm::Receipt;
use serde::{Deserialize, Serialize};

/// A proof produced by the host prover that bundles everything needed for
/// verification.
///
/// Wraps a `risc0_zkvm::Receipt`; verifiable only when its `inner` is
/// `InnerReceipt::Succinct`. Serialized via bincode legacy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Risc0Proof(pub Receipt);

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(Risc0Proof);

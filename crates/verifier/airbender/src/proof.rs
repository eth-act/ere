use airbender_execution_utils::ProgramProof;
use serde::{Deserialize, Serialize};

/// A proof produced by the host prover that bundles everything needed for
/// verification.
///
/// Wraps a `ProgramProof`; serialized via bincode legacy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AirbenderProof(pub ProgramProof);

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(AirbenderProof);

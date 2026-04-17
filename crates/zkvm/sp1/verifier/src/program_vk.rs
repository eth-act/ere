use sp1_hypercube::SP1VerifyingKey;

use serde::{Deserialize, Serialize};

/// Verifying key that identifies a specific compiled guest program.
///
/// Produced during setup and consumed by [`SP1Verifier`] together with a
/// [`SP1Proof`] to authenticate that the proof was generated from the same
/// program. Wraps an `SP1VerifyingKey` from `sp1-hypercube`; serialized via
/// bincode legacy.
///
/// [`SP1Verifier`]: crate::SP1Verifier
/// [`SP1Proof`]: crate::SP1Proof
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SP1ProgramVk(pub SP1VerifyingKey);

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(SP1ProgramVk);

use openvm_continuations::SC;
use serde::{Deserialize, Serialize};

use crate::vendor::VmStarkProof;

/// A proof produced by the host prover that bundles everything needed for
/// verification. Wraps a `VmStarkProof`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpenVMProof(pub VmStarkProof);

impl OpenVMProof {
    pub fn new(proof: openvm_continuations::verifier::internal::types::VmStarkProof<SC>) -> Self {
        Self(VmStarkProof {
            inner: proof.inner,
            user_public_values: proof.user_public_values,
        })
    }
}

ere_verifier_core::codec::impl_codec_by_bincode_legacy!(OpenVMProof);

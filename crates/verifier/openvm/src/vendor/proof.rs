use openvm_continuations::{F, SC};
use openvm_stark_sdk::openvm_stark_backend::proof::Proof;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct VmStarkProof {
    pub inner: Proof<SC>,
    pub user_public_values: Vec<F>,
}

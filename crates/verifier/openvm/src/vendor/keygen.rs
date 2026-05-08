use openvm_continuations::SC;
use openvm_stark_sdk::{
    config::FriParameters,
    openvm_stark_backend::{config::Com, keygen::types::MultiStarkVerifyingKey},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct AggVerifyingKey {
    pub leaf_fri_params: FriParameters,
    pub leaf_vk: MultiStarkVerifyingKey<SC>,
    pub internal_fri_params: FriParameters,
    pub internal_vk: MultiStarkVerifyingKey<SC>,
    pub internal_verifier_program_commit: Com<SC>,
}

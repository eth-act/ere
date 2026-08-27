#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cost;
mod error;
mod input;
mod prover;
mod resource;

pub use ere_codec as codec;
pub use ere_verifier_core::{PublicValues, zkVMVerifier};

pub use crate::{
    cost::{
        CostEstimation, ERE_COST_ESTIMATION_HEAP_END, ERE_COST_ESTIMATION_HEAP_START,
        symbol_address,
    },
    error::CommonError,
    input::Input,
    prover::{ProgramVk, Proof, zkVMProver},
    resource::{ProverResource, ProverResourceKind, RemoteProverConfig},
};

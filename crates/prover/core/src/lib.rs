#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cost;
mod error;
mod input;
mod prover;
mod resource;

#[cfg(test)]
mod test;

pub use ere_codec as codec;
pub use ere_compiler_core::Elf;
pub use ere_verifier_core::{PublicValues, zkVMVerifier};

pub use crate::{
    cost::{
        CallTree, CostEstimation, CostProfile, Frame, PeakMemory, RasAction, StackPointerWrite,
        SymbolMap, loadable_segments, pprof,
    },
    error::CommonError,
    input::Input,
    prover::{ProgramVk, Proof, zkVMProver},
    resource::{ProverResource, ProverResourceKind, RemoteProverConfig},
};

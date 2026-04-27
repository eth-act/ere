#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod input;
mod prover;
mod report;
mod resource;

pub use ere_codec as codec;
pub use ere_verifier_core::{PublicValues, zkVMVerifier};

pub use crate::{
    error::CommonError,
    input::Input,
    prover::{ProgramVk, Proof, zkVMProver},
    report::{ProgramExecutionReport, ProgramProvingReport},
    resource::{ProverResource, ProverResourceKind, RemoteProverConfig},
};

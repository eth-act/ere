#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod error;
pub mod input;
pub mod prover;
pub mod report;
pub mod resource;

pub use ere_codec as codec;
pub use ere_verifier_core::{PublicValues, zkVMVerifier};

pub use crate::{
    error::CommonError,
    input::Input,
    prover::{ProgramVk, Proof, zkVMProver},
    report::{ProgramExecutionReport, ProgramProvingReport},
    resource::{ProverResource, ProverResourceKind, RemoteProverConfig},
};

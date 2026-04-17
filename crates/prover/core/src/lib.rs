#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![allow(non_camel_case_types)]

pub mod error;
pub mod input;
pub mod prover;
pub mod report;
pub mod resource;

#[cfg(feature = "tokio")]
pub mod tokio;

pub use ere_codec as codec;
pub use ere_verifier_core::{PublicValues, zkVMVerifier};
pub use error::CommonError;
pub use input::Input;
pub use prover::{ProgramVk, Proof, zkVMProver};
pub use report::{ProgramExecutionReport, ProgramProvingReport};
pub use resource::{ProverResource, ProverResourceKind, RemoteProverConfig};
#[cfg(feature = "tokio")]
pub use tokio::block_on;

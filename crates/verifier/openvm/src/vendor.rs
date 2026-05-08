//! Vendor of crate `openvm-sdk` needed for verifeir to avoid pulling in `openvm` as dependency.

mod commit;
mod keygen;
mod proof;
mod verify;

pub use crate::vendor::{
    commit::{AppExecutionCommit, CommitBytes},
    keygen::AggVerifyingKey,
    proof::VmStarkProof,
    verify::verify_proof,
};

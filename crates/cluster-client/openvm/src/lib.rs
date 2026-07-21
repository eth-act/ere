//! Axiom Edge distributed cluster HTTP client for OpenVM programs.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod api;
mod client;
mod error;

#[cfg(test)]
mod test;

pub use ere_prover_core::*;
pub use ere_verifier_openvm::*;

pub use crate::{api::ProgramRef, client::OpenVMClusterClient, error::Error};

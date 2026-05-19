//! ZisK distributed cluster gRPC client.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

#[allow(clippy::enum_variant_names, clippy::large_enum_variant, dead_code)]
#[rustfmt::skip]
mod api;

mod client;
mod error;

#[cfg(test)]
mod test;

pub use ere_prover_core::*;
pub use ere_verifier_zisk::*;

pub use crate::{client::ZiskClusterClient, error::Error};

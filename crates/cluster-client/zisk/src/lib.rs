//! ZisK distributed cluster gRPC client.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

#[rustfmt::skip]
#[allow(clippy::enum_variant_names)]
mod api;

mod client;
mod error;

#[cfg(test)]
mod test;

pub use client::ZiskClusterClient;
pub use error::Error;

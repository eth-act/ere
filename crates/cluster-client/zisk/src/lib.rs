//! ZisK distributed cluster gRPC client.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod client;
mod error;

pub use client::ZiskClusterClient;
pub use error::Error;

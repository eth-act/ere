//! OpenVM [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_openvm_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-openvm` dependency.
//!
//! To use with GPU proving support, make sure CUDA 12.9 is installed, and turn
//! on the `cuda` feature.
//!
//! ## `zkVMProver` requirements
//!
//! - `cargo-openvm`
//! - Setup via `cargo openvm setup` - Setup aggregation keys used by
//!   `zkVMProver::prove`
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-openvm`](https://github.com/eth-act/ere/tree/master/crates/compiler/openvm) crate.
//!
//! # `zkVMProver` implementation
//!
//! ## Supported `ProverResource`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    Yes    |
//! | `Gpu`     |    Yes    |
//! | `Network` |    No     |
//! | `Cluster` |    No     |
//!
//! [`install_openvm_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_openvm_sdk.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod prover;

pub use prover::*;

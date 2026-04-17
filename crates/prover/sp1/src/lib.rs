//! SP1 [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_sp1_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-sp1` dependency.
//!
//! ## `zkVMProver` requirements
//!
//! - `docker` - Used by `zkVMProver::prove` if `ProverResource::Gpu` is selected
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-sp1`](https://github.com/eth-act/ere/tree/master/crates/compiler/sp1) crate.
//!
//! # `zkVMProver` implementation
//!
//! ## Supported `ProverResource`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    Yes    |
//! | `Gpu`     |    Yes    |
//! | `Network` |    Yes    |
//! | `Cluster` |    No     |
//!
//! [`install_sp1_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_sp1_sdk.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod prover;

pub use prover::*;

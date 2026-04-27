//! Risc0 [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_risc0_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-risc0` dependency.
//!
//! To install `r0vm-cuda` (with GPU proving support), make sure CUDA 12.9 is
//! installed, run [`install_risc0_sdk.sh`] with env `CUDA=1` set.
//!
//! ## `zkVMProver` requirements
//!
//! - [`rzup`]
//! - Installation via `rzup install`
//! - `r0vm-cuda` - Used by `zkVMProver::prove` if `ProverResource::Gpu` is selected
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-risc0`](https://github.com/eth-act/ere/tree/master/crates/compiler/risc0) crate.
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
//! [`install_risc0_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_risc0_sdk.sh
//! [`rzup`]: https://risczero.com/install

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod prover;

pub use ere_prover_core::*;
pub use ere_verifier_risc0::*;

pub use crate::{error::Error, prover::Risc0Prover};

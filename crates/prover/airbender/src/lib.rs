//! Airbender [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_airbender_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-airbender` dependency.
//!
//! To install `airbender-cli` with GPU proving support, make sure CUDA 12.9 is
//! installed, and run [`install_airbender_sdk.sh`] with env `CUDA=1` set.
//!
//! ## `zkVMProver` requirements
//!
//! The `zkVMProver` implementation requires external tools installed and available in
//! `PATH`:
//!
//! - `objcopy` (from `binutils`) - Used to convert ELF to binary at zkVMProver construction time
//! - Installation via [`install_airbender_sdk.sh`] - `airbender-cli` used by `zkVMProver::execute`
//!   and `zkVMProver::prove`
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-airbender`](https://github.com/eth-act/ere/tree/master/crates/compiler/airbender) crate.
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
//! [`install_airbender_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_airbender_sdk.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod prover;
mod sdk;

pub use ere_prover_core::*;
pub use ere_verifier_airbender::*;

pub use crate::{error::Error, prover::AirbenderProver};

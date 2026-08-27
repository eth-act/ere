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
//! ## Cost estimation
//!
//! The unit is `3 * trace_area + complexity`. The trace area counts the cells the
//! prover must fill, and the complexity is a weight SP1 gives each chip. SP1 gas
//! is this value divided by 10.
//!
//! | Component | Meaning                                               |
//! | --------- | ----------------------------------------------------- |
//! | `opcode`  | Plain RISC-V instructions                             |
//! | `syscall` | Syscalls, including the accelerated chips they invoke |
//! | `system`  | Chips SP1 runs for every program, such as memory      |
//!
//! Syscall and system costs are chip row counts times the gas weight of each
//! chip. The opcode cost is the total minus the other two.
//!
//! `peak_heap_bytes` covers guest memory above the `_end` symbol, or is `None`
//! when the estimator cannot read the heap.
//!
//! [`install_sp1_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_sp1_sdk.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cost;
mod error;
mod executor;
mod prover;
mod sdk;

pub use ere_prover_core::*;
pub use ere_verifier_sp1::*;

pub use crate::{error::Error, prover::SP1Prover};

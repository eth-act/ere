//! SP1 [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_sp1_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-sp1` dependency.
//!
//! GPU proving requires the `cuda` Cargo feature.
//!
//! ## `zkVMProver` requirements
//!
//! - `sp1-gpu-server` in `$HOME/.sp1/bin` - Used by `zkVMProver::prove` if `ProverResource::Gpu` is
//!   selected. The installer downloads it when `CUDA` is set, and the SP1 SDK downloads it when it
//!   is absent.
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
//! ## Preprocess
//!
//! `preprocess` compiles the program with the JIT in the tracing mode of a prove. It keeps nothing,
//! because SP1 has no disk cache for the compiled program.
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
//! ## Profile
//!
//! `profile` splits the cost estimate over the guest call stacks, in the same unit, and its
//! `cost_estimation` equals the `execute_estimated_cost` result. The frames follow
//! [`ere_prover_core::CallTree`]. SP1 does not prove `unconstrained!` blocks, so the profile does
//! not count their cost, memory or stack. `profile` fails for programs with untrusted programs.
//!
//! ## Environment variables
//!
//! | Variable                                | Type  | Default                                        | Description                                                   |
//! | --------------------------------------- | ----- | ---------------------------------------------- | ------------------------------------------------------------- |
//! | `ERE_SP1_EXECUTE_CONCURRENCY`           | Value | CPUs, max 32                                   | Executions that run at once                                   |
//! | `ERE_SP1_EXECUTE_ESTIMATED_CONCURRENCY` | Value | What free memory fits, at most the value above | Cost estimates and profiles that run at once, on x86_64 Linux |
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

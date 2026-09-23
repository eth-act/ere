//! LambdaVM [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_lambdavm_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-lambdavm` dependency.
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-lambdavm`](https://github.com/eth-act/ere/tree/master/crates/compiler/lambdavm) crate.
//!
//! # `zkVMProver` implementation
//!
//! ## Supported `ProverResource`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    Yes    |
//! | `Gpu`     |    No     |
//! | `Network` |    No     |
//! | `Cluster` |    No     |
//!
//! ## Security
//!
//! Proofs are generated and verified with FRI blowup factor [`BLOWUP_FACTOR`]: 128-bit target,
//! 20 grinding bits, FRI query count from the Johnson bound regime.
//!
//! Proofs are not zero-knowledge. A proof opens trace columns at the query positions, so it
//! does not hide the private input.
//!
//! ## Cost estimation
//!
//! | Component       | Meaning                                                   |
//! | --------------- | --------------------------------------------------------- |
//! | `cycles`        | Executed RISC-V instructions                              |
//! | `main_elements` | Field elements of the main traces of all tables           |
//! | `aux_elements`  | Field elements of the auxiliary (LogUp) traces of all tables |
//!
//! Execution fails if it runs longer than 2^32 cycles. `execute` runs in chunks, so its memory use
//! does not grow with the cycle count. The element counts and `prove` keep one log per cycle in
//! memory, because `lambda-vm-prover` executes the whole program at once.
//!
//! `peak_heap_bytes` spans from the `_end` symbol up to the highest non-zero byte below the top of
//! the guest heap at `0xC0000000`, or is `None` when the estimator cannot find `_end`.
//!
//! [`install_lambdavm_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_lambdavm_sdk.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cost;
mod error;
mod executor;
mod prover;

pub use ere_prover_core::*;
pub use ere_verifier_lambdavm::*;

pub use crate::{error::Error, prover::LambdaVMProver};

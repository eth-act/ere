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
//! - Setup via `cargo openvm setup` - Setup aggregation keys used by `zkVMProver::prove`
//! - LLVM clang 19 or newer, `lld` and `make`, used by OpenVM's `rvr` backend to compile each guest
//!   program to a shared library when the prover is constructed
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
//! ## Cost estimation
//!
//! The unit is trace cells. A table costs its rows times its width. The count is
//! unpadded, because the prover rounds each table up to a power of two rows.
//!
//! | Component    | Meaning                                                         |
//! | ------------ | --------------------------------------------------------------- |
//! | `rv64`       | Plain RISC-V instructions                                       |
//! | `precompile` | Accelerated chips, such as Keccak, SHA-2 and modular arithmetic |
//! | `system`     | Tables both groups share, plus fixed VM overhead                |
//!
//! A long run splits into segments, and a table of fixed height is paid once per
//! segment. `ERE_OPENVM_SEGMENT_MEMORY` sets the limit that starts a new segment,
//! by default 14.5 GiB.
//!
//! `peak_heap_bytes` spans the non-zero bytes above the `_end` symbol, or is
//! `None` when the estimator cannot read the heap.
//!
//! [`install_openvm_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_openvm_sdk.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cost;
mod error;
mod executor;
mod prover;

pub use ere_prover_core::*;
pub use ere_verifier_openvm::*;

pub use crate::{error::Error, prover::OpenVMProver};

//! OpenVM [`Compiler`] and [`zkVMProver`] implementation.
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
//! ## Available compilers
//!
//! | Compiler                | Language | Target                        | Note               |
//! | ----------------------- | :------: | ----------------------------- | ------------------ |
//! | `RustRv32imaCustomized` |   Rust   | `riscv32im-risc0-zkvm-elf`    | With `std` support |
//! | `RustRv32ima`           |   Rust   | `riscv32ima-unknown-none-elf` |                    |
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

#![cfg_attr(
    all(not(test), feature = "compiler", feature = "zkvm"),
    warn(unused_crate_dependencies)
)]

#[cfg(feature = "compiler")]
pub mod compiler;

#[cfg(feature = "zkvm")]
pub mod prover;

#[cfg(feature = "zkvm")]
pub use prover::*;

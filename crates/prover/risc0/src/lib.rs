//! Risc0 [`Compiler`] and [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_risc0_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-risc0` dependency.
//!
//! To install `r0vm-cuda` (with GPU proving support), make sure CUDA 12.9 is
//! installed, run [`install_risc0_sdk.sh`] with env `CUDA=1` set.
//!
//! ## `Compiler` requirements
//!
//! - [`rzup`]
//! - Installation via `rzup install` - Custom Rust toolchain used by `RustRv32imaCustomized`
//!
//! ## `zkVMProver` requirements
//!
//! - [`rzup`]
//! - Installation via `rzup install`
//! - `r0vm-cuda` - Used by `zkVMProver::prove` if `ProverResource::Gpu` is
//!   selected
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
//! [`install_risc0_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_risc0_sdk.sh
//! [`rzup`]: https://risczero.com/install

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

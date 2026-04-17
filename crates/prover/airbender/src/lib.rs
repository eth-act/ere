//! Airbender [`Compiler`] and [`zkVMProver`] implementation.
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
//! - Installation via [`install_airbender_sdk.sh`] - `airbender-cli` used by
//!   `zkVMProver::execute` and `zkVMProver::prove`
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler      | Language | Target                        |
//! | ------------- | :------: | ----------------------------- |
//! | `RustRv32ima` |   Rust   | `riscv32ima-unknown-none-elf` |
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

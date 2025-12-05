//! Airbender [`Compiler`] and [`zkVM`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_airbender_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-airbender` dependency.
//!
//! To install `airbender-cli` with GPU proving support, make sure CUDA 12.9 is
//! installed, and run [`install_airbender_sdk.sh`] with env `CUDA=1` set.
//!
//! ## `Compiler` requirements
//!
//! The `Compiler` implementation requires external tools installed and
//! available in `PATH`:
//!
//! - `rust-objcopy` - Used by compiler to convert ELF to binary
//!
//! ## `zkVM` requirements
//!
//! The `zkVM` implementation requires external tools installed and available in
//! `PATH`:
//!
//! - Installation via [`install_airbender_sdk.sh`] - `airbender-cli` used by
//!   `zkVM::execute` and `zkVM::prove`
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler      | Language | Target                        |
//! | ------------- | :------: | ----------------------------- |
//! | `RustRv32ima` |   Rust   | `riscv32ima-unknown-none-elf` |
//!
//! # `zkVM` implementation
//!
//! ## Supported `ProverResourceType`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    Yes    |
//! | `Gpu`     |    Yes    |
//! | `Network` |    No     |
//!
//! [`install_airbender_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_airbender_sdk.sh

#![cfg_attr(
    all(not(test), feature = "compiler", feature = "zkvm"),
    warn(unused_crate_dependencies)
)]

pub mod program;

#[cfg(feature = "compiler")]
pub mod compiler;

#[cfg(feature = "zkvm")]
pub mod zkvm;

#[cfg(feature = "zkvm")]
pub use zkvm::*;

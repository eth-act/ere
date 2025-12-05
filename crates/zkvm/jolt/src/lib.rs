//! Jolt [`Compiler`] and [`zkVM`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_jolt_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-jolt` dependency.
//!
//! ## `Compiler` requirements
//!
//! - `jolt`
//! - Install custom Rust toolchain via `jolt install-toolchain` - Used by `RustRv64imaCustomized`
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler                  | Language | Target                         | Note               |
//! | ------------------------- | :------: | ------------------------------ | ------------------ |
//! | `RustRv64imacCustomized`  |   Rust   | `riscv64imac-jolt-zkvm-elf`    | With `std` support |
//! | `RustRv64imac`            |   Rust   | `riscv64imac-unknown-none-elf` |                    |
//!
//! # `zkVM` implementation
//!
//! ## Supported `ProverResourceType`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    Yes    |
//! | `Gpu`     |    No     |
//! | `Network` |    No     |
//!
//! [`install_jolt_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_jolt_sdk.sh

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

//! Ziren [`Compiler`] and [`zkVM`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_ziren_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-ziren` dependency.
//!
//! ## `Compiler` requirements
//!
//! - Installation via [`install_ziren_sdk.sh`] - Custom Rust toolchain used by `RustMips32r2Customized`
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler                 | Language | Target                | Note               |
//! | ------------------------ | :------: | --------------------- | ------------------ |
//! | `RustMips32r2Customized` |   Rust   | `mipsel-zkm-zkvm-elf` | With `std` support |
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
//! [`install_ziren_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_ziren_sdk.sh

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

//! Pico [`Compiler`] and [`zkVM`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_pico_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-pico` dependency.
//!
//! ## `Compiler` requirements
//!
//! - `cargo-pico` - Used by `RustRv32imaCustomized`
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
//! [`install_pico_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_pico_sdk.sh

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

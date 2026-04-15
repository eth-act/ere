//! SP1 [`Compiler`] and [`zkVM`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_sp1_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-sp1` dependency.
//!
//! ## `Compiler` requirements
//!
//! - Installation via [`sp1up`] - Custom Rust toolchain used by `RustRv64imaCustomized`
//! - `cargo-prove` - Used by `RustRv64imaCustomized`
//!
//! ## `zkVM` requirements
//!
//! - `docker` - Used by `zkVM::prove` if `ProverResource::Gpu` is selected
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler                | Language | Target                        | Note               |
//! | ----------------------- | :------: | ----------------------------- | ------------------ |
//! | `RustRv64imaCustomized` |   Rust   | `riscv64im-succinct-zkvm-elf` | With `std` support |
//! | `RustRv64ima`           |   Rust   | `riscv64ima-unknown-none-elf` |                    |
//!
//! # `zkVM` implementation
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
//! [`install_sp1_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_sp1_sdk.sh
//! [`sp1up`]: https://sp1up.succinct.xyz

#![cfg_attr(
    all(not(test), feature = "compiler", feature = "zkvm"),
    warn(unused_crate_dependencies)
)]

#[cfg(feature = "compiler")]
pub mod compiler;

#[cfg(feature = "zkvm")]
pub mod zkvm;

#[cfg(feature = "zkvm")]
pub use zkvm::*;

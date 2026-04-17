//! ZisK [`Compiler`] and [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_zisk_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-zisk` dependency.
//!
//! GPU proving requires the `cuda` Cargo feature and CUDA 12.9 installed.
//!
//! ## `Compiler` requirements
//!
//! - Installation via [`ziskup`] - Custom Rust toolchain used by `RustRv64imaCustomized`
//! - Installation via [`install_tamago.sh`] - Custom Go toolchain used by `GoCustomized`
//!
//! ## `zkVMProver` requirements
//!
//! - Installation via [`ziskup`]
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler                | Language | Target                        | Note               |
//! | ----------------------- | :------: | ----------------------------- | ------------------ |
//! | `RustRv64imaCustomized` |   Rust   | `riscv64ima-zisk-zkvm-elf`    | With `std` support |
//! | `RustRv64ima`           |   Rust   | `riscv64ima-unknown-none-elf` |                    |
//! | `GoCustomized`          |    Go    | `riscv64`                     |                    |
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
//! | `Cluster` |    Yes    |
//!
//! ## Environment variables
//!
//! | Variable                               | Type  | Default | Description                                                            |
//! | -------------------------------------- | ----- | ------- | ---------------------------------------------------------------------- |
//! | `ERE_ZISK_SETUP_ON_INIT`               | Flag  |         | Setup local prover on initialization instead of lazily                 |
//! | `ERE_ZISK_UNLOCK_MAPPED_MEMORY`        | Flag  |         | Configure the prover to unlock mapped memory                           |
//! | `ERE_ZISK_MINIMAL_MEMORY`              | Flag  |         | Configure the prover to use minimal memory                             |
//! | `ERE_ZISK_PREALLOCATE`                 | Flag  |         | Configure the prover to preallocate memory                             |
//! | `ERE_ZISK_SHARED_TABLES`               | Flag  |         | Configure the prover to use shared tables                              |
//! | `ERE_ZISK_MAX_STREAMS`                 | Value |         | Configure the prover max streams                                       |
//! | `ERE_ZISK_NUMBER_THREADS_WITNESS`      | Value |         | Configure the prover number of witness threads                         |
//! | `ERE_ZISK_MAX_WITNESS_STORED`          | Value |         | Configure the prover max witness stored                                |
//!
//! [`install_zisk_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_zisk_sdk.sh
//! [`install_tamago.sh`]: https://github.com/eth-act/ere/blob/master/scripts/install_tamago.sh
//! [`ziskup`]: https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh

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

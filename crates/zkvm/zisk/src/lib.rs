//! ZisK [`Compiler`] and [`zkVM`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_zisk_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-zisk` dependency.
//!
//! To install `cargo-zisk-cuda` (with GPU proving support), make sure CUDA 12.9
//! is installed, run [`install_zisk_sdk.sh`] with env `CUDA=1` set.
//!
//! ## `Compiler` requirements
//!
//! - Installation via [`ziskup`] - Custom Rust toolchain used by `RustRv64imaCustomized`
//! - Installation via [`install_tamago.sh`] - Custom Go toolchain used by `GoCustomized`
//!
//! ## `zkVM` requirements
//!
//! - Installation via [`ziskup`]
//! - `cargo-zisk-cuda` - Used by `zkVM::prove` if `ProverResource::Gpu` is
//!   selected
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler                | Language | Target                     | Note               |
//! | ----------------------- | :------: | -------------------------- | ------------------ |
//! | `RustRv64imaCustomized` |   Rust   | `riscv64ima-zisk-zkvm-elf` | With `std` support |
//! | `GoCustomized`          |    Go    | `riscv64`                  |                    |
//!
//! # `zkVM` implementation
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
//! | `ERE_ZISK_SETUP_ON_INIT`               | Flag  |         | Trigger ROM setup at initialization instead of lazily                  |
//! | `ERE_ZISK_PORT`                        | Value |         | Pass `--port {port}` to the server                                     |
//! | `ERE_ZISK_UNLOCK_MAPPED_MEMORY`        | Flag  |         | Pass `--unlock-mapped-memory` to the server                            |
//! | `ERE_ZISK_MINIMAL_MEMORY`              | Flag  |         | Pass `--minimal_memory` to the server                                  |
//! | `ERE_ZISK_PREALLOCATE`                 | Flag  |         | Pass `--preallocate` to the server                                     |
//! | `ERE_ZISK_SHARED_TABLES`               | Flag  |         | Pass `--shared-tables` to the server                                   |
//! | `ERE_ZISK_MAX_STREAMS`                 | Value |         | Pass `--max-streams {max_streams}` to the server                       |
//! | `ERE_ZISK_NUMBER_THREADS_WITNESS`      | Value |         | Pass `--number-threads-witness {number_threads_witness}` to the server |
//! | `ERE_ZISK_MAX_WITNESS_STORED`          | Value |         | Pass `--max-witness-stored {max_witness_stored}` to the server         |
//! | `ERE_ZISK_START_SERVER_TIMEOUT_SEC`    | Value | `120`   | Timeout waiting for server to start                                    |
//! | `ERE_ZISK_SHUTDOWN_SERVER_TIMEOUT_SEC` | Value | `30`    | Timeout for server shutdown                                            |
//! | `ERE_ZISK_PROVE_TIMEOUT_SEC`           | Value | `3600`  | Timeout for proof generation                                           |
//!
//! [`install_zisk_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_zisk_sdk.sh
//! [`install_tamago.sh`]: https://github.com/eth-act/ere/blob/master/scripts/install_tamago.sh
//! [`ziskup`]: https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh

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

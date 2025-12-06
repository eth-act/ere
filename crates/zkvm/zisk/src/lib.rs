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
//! - `cargo-zisk-cuda` - Used by `zkVM::prove` if `ProverResourceType::Gpu` is
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
//! ## Supported `ProverResourceType`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    Yes    |
//! | `Gpu`     |    Yes    |
//! | `Network` |    No     |
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

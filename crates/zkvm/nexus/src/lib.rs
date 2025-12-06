//! Nexus [`Compiler`] and [`zkVM`] implementation.
//!
//! # `Compiler` implementation
//!
//! ## Available compilers
//!
//! | Compiler     | Language | Target                      |
//! | ------------ | :------: | --------------------------- |
//! | `RustRv32i`  |   Rust   | `riscv32i-unknown-none-elf` |
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

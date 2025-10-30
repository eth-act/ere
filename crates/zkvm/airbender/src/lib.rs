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

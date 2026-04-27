#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod rust_rv32ima;
mod rust_rv32ima_customized;

pub use ere_compiler_core::*;

pub use crate::{
    error::Error, rust_rv32ima::Risc0RustRv32ima,
    rust_rv32ima_customized::Risc0RustRv32imaCustomized,
};

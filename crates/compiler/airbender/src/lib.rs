#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod rust_rv32ima;
mod rust_rv32ima_customized;

pub use ere_compiler_core::*;

pub use crate::{
    error::Error, rust_rv32ima::AirbenderRustRv32ima,
    rust_rv32ima_customized::AirbenderRustRv32imaCustomized,
};

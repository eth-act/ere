#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod rust_rv32ima;

pub use ere_compiler_core::*;

pub use crate::{error::Error, rust_rv32ima::AirbenderRustRv32ima};

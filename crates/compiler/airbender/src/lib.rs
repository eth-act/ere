#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod rust_rv32ima;

pub use error::Error;
pub use rust_rv32ima::AirbenderRustRv32ima;

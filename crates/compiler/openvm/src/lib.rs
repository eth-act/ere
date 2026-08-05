#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod rust_rv64ima;
mod rust_rv64ima_customized;

pub use ere_compiler_core::*;

pub use crate::{
    error::Error, rust_rv64ima::OpenVMRustRv64ima,
    rust_rv64ima_customized::OpenVMRustRv64imaCustomized,
};

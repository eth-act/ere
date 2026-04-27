#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod rust;

pub use crate::{
    error::CommonError,
    rust::{
        CargoBuildCmd, RustTarget, cargo_metadata, rustc_path, rustup_active_toolchain,
        rustup_add_components, rustup_add_rust_src, rustup_add_target,
    },
};

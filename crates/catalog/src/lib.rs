#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod compiler;
pub mod zkvm;

pub use crate::{compiler::CompilerKind, zkvm::zkVMKind};

include!(concat!(env!("OUT_DIR"), "/docker_image_tag.rs"));
include!(concat!(env!("OUT_DIR"), "/zkvm_sdk_version_impl.rs"));

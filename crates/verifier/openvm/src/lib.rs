#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod program_vk;
mod proof;
mod verifier;

pub use ere_verifier_core::*;

pub use crate::{
    error::Error,
    program_vk::OpenVMProgramVk,
    proof::OpenVMProof,
    verifier::{
        NUM_PUBLIC_VALUES, OpenVMVerifier, extract_public_values, vm_config::sdk_vm_config,
    },
};

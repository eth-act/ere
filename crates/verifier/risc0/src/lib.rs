#![no_std]

extern crate alloc;

mod error;
mod program_vk;
mod proof;
mod verifier;

pub use ere_verifier_core::*;

pub use crate::{
    error::Error, program_vk::Risc0ProgramVk, proof::Risc0Proof, verifier::Risc0Verifier,
};

mod error;
mod program_vk;
mod proof;
mod verifier;

pub use ere_verifier_core::*;

pub use crate::{
    error::Error,
    program_vk::ZiskProgramVk,
    proof::{
        PROGRAM_VK_WORDS, PUBLIC_VALUES_BYTES, PUBLIC_VALUES_WORDS, VadcopFinalProof, ZiskProof,
    },
    verifier::{VADCOP_FINAL_HASH_FAMILY, ZiskVerifier, ensure_program_vk_matches},
};

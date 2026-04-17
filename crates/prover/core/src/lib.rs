#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod prover;

pub use ere_codec as codec;
pub use prover::*;

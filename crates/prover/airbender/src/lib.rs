//! Airbender [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! - `objcopy` (from `binutils`) - Used to convert ELF to binary/text at zkVMProver construction
//!   time
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-airbender`](https://github.com/eth-act/ere/tree/master/crates/compiler/airbender) crate.
//!
//! # `zkVMProver` implementation
//!
//! ## Supported `ProverResource`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    No     |
//! | `Gpu`     |    Yes    |
//! | `Network` |    No     |
//! | `Cluster` |    No     |

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod prover;

pub use ere_prover_core::*;
pub use ere_verifier_airbender::*;

pub use crate::{error::Error, prover::AirbenderProver};

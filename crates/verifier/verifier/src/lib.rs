//! Unified verifier for the zkVMs supported by `ere`.
//!
//! [`Verifier`] dispatches to the per-zkVM verifier crates based on a
//! [`zkVMKind`] discriminant. It takes byte-encoded program verifying keys
//! and proofs, decodes them through the codec re-exported by
//! [`ere-verifier-core`], and returns [`PublicValues`] on success.
//!
//! # Example
//!
//! ```rust,no_run
//! use ere_verifier::{Verifier, zkVMKind};
//!
//! # fn run(encoded_program_vk: &[u8], encoded_proof: &[u8])
//! #     -> Result<(), ere_verifier::Error>
//! # {
//! let verifier = Verifier::new(zkVMKind::SP1, encoded_program_vk)?;
//! let public_values = verifier.verify(encoded_proof)?;
//! # Ok(()) }
//! ```
//!
//! [`zkVMKind`]: ere_catalog::zkVMKind
//! [`PublicValues`]: ere_verifier_core::PublicValues
//! [`ere-verifier-core`]: https://github.com/eth-act/ere/tree/master/crates/verifier/core

mod error;
mod verifier;

pub use ere_catalog::zkVMKind;

pub use crate::{error::Error, verifier::Verifier};

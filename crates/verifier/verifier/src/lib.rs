//! Unified verifier for the zkVMs supported by `ere`.
//!
//! [`Verifier`] dispatches to the per-zkVM verifier crates based on a
//! [`zkVMKind`] discriminant. It takes byte-encoded program verifying keys
//! and proofs, decodes them through the codec re-exported by
//! [`ere-verifier-core`], and returns [`PublicValues`] on success.
//!
//! # Feature flags
//!
//! - `nightly` *(off by default)* — Enables the `Airbender` variant of [`Verifier`] by pulling in
//!   [`ere-verifier-airbender`]. Airbender's host SDK depends on nightly-only language features, so
//!   the feature also requires building with a nightly Rust toolchain.
//!
//! Without `nightly`, [`Verifier::new`] returns [`Error::NightlyFeatureRequired`] when called with
//! the `Airbender` variant of [`zkVMKind`]. To verify proofs from every supported zkVM, enable
//! `nightly` and build with a nightly toolchain.
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
//! [`ere-verifier-airbender`]: https://github.com/eth-act/ere/tree/master/crates/verifier/airbender

mod error;
mod verifier;

pub use ere_catalog::zkVMKind;

pub use crate::{error::Error, verifier::Verifier};

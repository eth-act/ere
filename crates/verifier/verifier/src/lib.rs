//! Unified verifier for the zkVMs supported by `ere`.
//!
//! [`Verifier`] dispatches to the per-zkVM verifier crates based on a
//! [`zkVMKind`] discriminant. It takes byte-encoded program verifying keys
//! and proofs, decodes them through the codec re-exported by
//! [`ere-verifier-core`], and returns [`PublicValues`] on success.
//!
//! # Example
//!
//! A proof that verifies only shows that the program ran. The public values carry the result, so
//! the caller compares them with the expected bytes and treats a difference as a rejected proof.
//!
//! ```rust,no_run
//! use ere_verifier::{Verifier, zkVMKind};
//!
//! # fn run(encoded_program_vk: &[u8], encoded_proof: &[u8], expected_public_values: &[u8])
//! #     -> Result<bool, ere_verifier::Error>
//! # {
//! let verifier = Verifier::new(zkVMKind::SP1, encoded_program_vk)?;
//! let public_values = verifier.verify(encoded_proof)?;
//! let accepted = *public_values == *expected_public_values;
//! # Ok(accepted) }
//! ```
//!
//! # Public values
//!
//! The layout of the public values depends on the zkVM kind.
//!
//! - [`zkVMKind::SP1`], [`zkVMKind::LambdaVM`] and [`zkVMKind::Pico`] return the exact bytes that
//!   the guest wrote. The caller compares them with the expected bytes exactly, length included.
//! - [`zkVMKind::OpenVM`] and [`zkVMKind::Zisk`] return 256 bytes, which are the bytes that the
//!   guest wrote followed by zeros. The caller pads its expected bytes with zeros to 256 bytes and
//!   then compares exactly.
//!
//! # Panics
//!
//! [`Verifier::new`] and [`Verifier::verify`] catch a panic of the verifier code and return it as
//! [`Error::Panic`]. A caller treats that error as a rejected proof. The guard does not cover an
//! abort from a failed allocation, a stack overflow, or a build with `panic = "abort"`, because
//! each of these ends the process without an unwind. The panic hook of the process still runs for a
//! caught panic, so the default hook prints the panic message to standard error.
//!
//! [`zkVMKind`]: ere_catalog::zkVMKind
//! [`PublicValues`]: ere_verifier_core::PublicValues
//! [`ere-verifier-core`]: https://github.com/eth-act/ere/tree/master/crates/verifier/core

mod error;
mod verifier;

pub use ere_catalog::zkVMKind;

pub use crate::{error::Error, verifier::Verifier};

//! Status codes returned by the C-ABI functions.
//!
//! These integer values form part of the public C ABI. Existing codes are
//! never renumbered or repurposed, and new variants take the next free value.

/// Operation succeeded.
pub const ERE_OK: i32 = 0;
/// A required pointer argument was null when a value was expected.
pub const ERE_ERR_NULL_PTR: i32 = 1;
/// `zkvm_kind` was not one of the documented values.
pub const ERE_ERR_BAD_KIND: i32 = 2;
/// The program verifying key bytes failed to decode.
pub const ERE_ERR_DECODE_PROGRAM_VK: i32 = 3;
/// The proof bytes failed to decode.
pub const ERE_ERR_DECODE_PROOF: i32 = 4;
/// The proof was well-formed but failed cryptographic verification.
pub const ERE_ERR_VERIFY: i32 = 5;
/// An unexpected internal condition occurred. This indicates a bug in the
/// binding or the verifier library rather than an invalid argument.
pub const ERE_ERR_INTERNAL: i32 = 6;

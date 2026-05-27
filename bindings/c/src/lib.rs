//! C ABI wrapper around [`ere_verifier::Verifier`].
//!
//! # Thread safety
//!
//! A handle may be shared across threads for concurrent [`ere_verifier_verify`]
//! and [`ere_verifier_zkvm_kind`] calls. [`ere_verifier_free`] consumes it and
//! must not overlap with other calls on the handle.

#![allow(non_camel_case_types)]

mod error;

use core::{ptr, slice};

use ere_verifier::{Error, zkVMKind};

pub use crate::error::{
    ERE_ERR_BAD_KIND, ERE_ERR_DECODE_PROGRAM_VK, ERE_ERR_DECODE_PROOF, ERE_ERR_INTERNAL,
    ERE_ERR_NULL_PTR, ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_LARGE,
    ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_SMALL, ERE_ERR_VERIFY, ERE_OK,
};

/// Opaque verifier handle returned by [`ere_verifier_new`] and released by
/// [`ere_verifier_free`].
pub struct EreVerifier(ere_verifier::Verifier);

/// Constructs a verifier bound to an encoded program verifying key.
///
/// The `zkvm_kind` includes:
/// - `0` - [`zkVMKind::Airbender`]
/// - `1` - [`zkVMKind::OpenVM`]
/// - `2` - [`zkVMKind::Risc0`]
/// - `3` - [`zkVMKind::SP1`]
/// - `4` - [`zkVMKind::Zisk`]
///
/// On success, writes the new handle into `*output` and returns [`ERE_OK`].
/// The caller owns the handle and must release it with [`ere_verifier_free`].
///
/// On error, `*output` is set to null and the corresponding status code
/// is returned.
///
/// # Safety
///
/// - `encoded_program_vk_ptr` must point to `encoded_program_vk_len` readable bytes (or be null
///   when `encoded_program_vk_len == 0`).
/// - `output` must be a non-null, writable `*mut *mut EreVerifier`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_new(
    zkvm_kind: u32,
    encoded_program_vk_ptr: *const u8,
    encoded_program_vk_len: usize,
    output: *mut *mut EreVerifier,
) -> i32 {
    if output.is_null() {
        return ERE_ERR_NULL_PTR;
    }
    unsafe { *output = ptr::null_mut() };

    let Some(kind) = u8::try_from(zkvm_kind).ok().and_then(zkVMKind::from_u8) else {
        return ERE_ERR_BAD_KIND;
    };
    let Some(encoded_program_vk) =
        (unsafe { as_slice(encoded_program_vk_ptr, encoded_program_vk_len) })
    else {
        return ERE_ERR_NULL_PTR;
    };

    match ere_verifier::Verifier::new(kind, encoded_program_vk) {
        Ok(verifier) => {
            let boxed = Box::new(EreVerifier(verifier));
            unsafe { *output = Box::into_raw(boxed) };
            ERE_OK
        }
        Err(Error::DecodeProgramVk(_)) => ERE_ERR_DECODE_PROGRAM_VK,
        Err(Error::NightlyFeatureRequired | Error::DecodeProof(_) | Error::Verification(_)) => {
            ERE_ERR_INTERNAL
        }
    }
}

/// Verifies a proof against the verifier's program verifying key and copies the
/// public values into the `public_values_ptr` buffer.
///
/// When the `public_values_ptr` buffer has length exactly as verified public values, or non-zero
/// leading part of it (accommodates proof systems that pad public values to a fixed length), the
/// verified public values are copied to `public_values_ptr` buffer and [`ERE_OK`] is returned.
///
/// A buffer longer than the public values returns [`ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_LARGE`], a
/// shorter buffer returns [`ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_SMALL`] unless the trailing bytes are
/// all zero.
///
/// # Safety
///
/// - `handle` must be a live handle returned by [`ere_verifier_new`].
/// - `encoded_proof_ptr` must point to `encoded_proof_len` readable bytes (or be null when
///   `encoded_proof_len == 0`).
/// - `public_values_ptr` must point to `public_values_len` writable bytes (or be null when
///   `public_values_len == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_verify(
    handle: *const EreVerifier,
    encoded_proof_ptr: *const u8,
    encoded_proof_len: usize,
    public_values_ptr: *mut u8,
    public_values_len: usize,
) -> i32 {
    if handle.is_null() {
        return ERE_ERR_NULL_PTR;
    }
    let Some(encoded_proof) = (unsafe { as_slice(encoded_proof_ptr, encoded_proof_len) }) else {
        return ERE_ERR_NULL_PTR;
    };
    if public_values_ptr.is_null() && public_values_len != 0 {
        return ERE_ERR_NULL_PTR;
    }

    let verifier = unsafe { &(*handle).0 };
    let public_values = match verifier.verify(encoded_proof) {
        Ok(public_values) => public_values.0,
        Err(Error::DecodeProof(_)) => return ERE_ERR_DECODE_PROOF,
        Err(Error::Verification(_)) => return ERE_ERR_VERIFY,
        Err(Error::NightlyFeatureRequired | Error::DecodeProgramVk(_)) => return ERE_ERR_INTERNAL,
    };

    if public_values_len > public_values.len() {
        return ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_LARGE;
    }
    let (public_values, trailing) = public_values.split_at(public_values_len);
    if trailing.iter().any(|&b| b != 0) {
        return ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_SMALL;
    }
    if public_values_len != 0 {
        unsafe {
            ptr::copy_nonoverlapping(public_values.as_ptr(), public_values_ptr, public_values_len)
        };
    }
    ERE_OK
}

/// Writes the `zkvm_kind` integer the verifier was constructed for into
/// `*output` and returns [`ERE_OK`].
///
/// # Safety
///
/// - `handle` must be a live handle returned by [`ere_verifier_new`].
/// - `output` must be a non-null, writable `*mut u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_zkvm_kind(
    handle: *const EreVerifier,
    output: *mut u32,
) -> i32 {
    if handle.is_null() || output.is_null() {
        return ERE_ERR_NULL_PTR;
    }
    unsafe { *output = (*handle).0.zkvm_kind().as_u32() };
    ERE_OK
}

/// Releases a verifier handle. The handle must not be used after this call.
/// Passing a null pointer is a no-op.
///
/// # Safety
///
/// `handle` must be a live handle returned by [`ere_verifier_new`] or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_free(handle: *mut EreVerifier) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Reconstructs a `&[u8]` from a `(ptr, len)` pair, treating `(NULL, 0)` as
/// the empty slice and returning `None` for `(NULL, len > 0)`.
///
/// # Safety
///
/// When `ptr` is non-null it must point to `len` initialised, readable bytes
/// that are not mutated for the returned slice's lifetime. The caller picks
/// `'a`. It must not outlive the underlying allocation.
unsafe fn as_slice<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        Some(&[])
    } else if ptr.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(ptr, len) })
    }
}

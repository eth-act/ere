//! C ABI wrapper around [`ere_verifier::Verifier`].
//!
//! # Thread safety
//!
//! A handle may be shared across threads for concurrent [`ere_verifier_verify`]
//! and [`ere_verifier_zkvm_kind`] calls. [`ere_verifier_free`] consumes it and
//! must not overlap with other calls on the handle.
//!
//! # Panics
//!
//! Every exported function catches a panic of its body, so no panic crosses the C boundary. A
//! function that returns a status returns [`ERE_ERR_PANIC`] for it, and a caller treats that status
//! as a rejected proof. The guard does not cover an abort from a failed allocation, a stack
//! overflow, or a build with `panic = "abort"`, because each of these ends the process without an
//! unwind. The panic hook of the process still runs for a caught panic, so the default hook prints
//! the panic message to standard error.

mod error;

use core::{ptr, slice};
use std::panic::{AssertUnwindSafe, catch_unwind};

use ere_verifier::{Error, Verifier, zkVMKind};

pub use crate::error::{
    ERE_ERR_BAD_KIND, ERE_ERR_DECODE_PROGRAM_VK, ERE_ERR_DECODE_PROOF, ERE_ERR_INTERNAL,
    ERE_ERR_NULL_PTR, ERE_ERR_PANIC, ERE_ERR_VERIFY, ERE_OK,
};

/// Opaque handle to a verifier bound to a program verifying key. Created by
/// [`ere_verifier_new`] and released by [`ere_verifier_free`].
pub struct EreVerifier(Verifier);

/// Constructs a verifier bound to an encoded program verifying key for the
/// selected zkVM.
///
/// `zkvm_kind` selects the target zkVM.
///
/// - `0` - [`zkVMKind::OpenVM`]
/// - `1` - [`zkVMKind::SP1`]
/// - `2` - [`zkVMKind::Zisk`]
/// - `3` - [`zkVMKind::LambdaVM`]
/// - `4` - [`zkVMKind::Pico`]
///
/// On success, writes the new handle into `*output` and returns [`ERE_OK`]. The
/// caller owns the handle and must release it with [`ere_verifier_free`]. On
/// error, `*output` is set to null and the corresponding status code is
/// returned. Only a null `output` leaves `*output` unwritten.
///
/// [`ERE_ERR_DECODE_PROGRAM_VK`] and [`ERE_ERR_PANIC`] both mean that the library rejects the key.
///
/// # Safety
///
/// - `encoded_program_vk_ptr` must point to `encoded_program_vk_len` readable bytes that do not
///   change during the call. It can be null when `encoded_program_vk_len == 0`. A length above
///   `PTRDIFF_MAX` returns [`ERE_ERR_NULL_PTR`].
/// - `output` must be a non-null, writable `*mut *mut EreVerifier`, aligned for a pointer, that
///   does not overlap the key bytes.
#[unsafe(no_mangle)]
#[must_use = "the status must be checked"]
pub unsafe extern "C" fn ere_verifier_new(
    zkvm_kind: u32,
    encoded_program_vk_ptr: *const u8,
    encoded_program_vk_len: usize,
    output: *mut *mut EreVerifier,
) -> i32 {
    guard(|| {
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

        match Verifier::new(kind, encoded_program_vk) {
            Ok(verifier) => {
                let boxed = Box::new(EreVerifier(verifier));
                unsafe { *output = Box::into_raw(boxed) };
                ERE_OK
            }
            Err(Error::DecodeProgramVk(_)) => ERE_ERR_DECODE_PROGRAM_VK,
            Err(Error::Panic(_)) => ERE_ERR_PANIC,
            Err(_) => ERE_ERR_INTERNAL,
        }
    })
}

/// Verifies a proof against the verifier's program verifying key and allocates a
/// buffer holding the proven public values.
///
/// On success, stores the buffer pointer into `*public_values_ptr` and its
/// length into `*public_values_len`, and returns [`ERE_OK`]. The caller owns the
/// buffer and must release it with [`ere_bytes_free`], passing back the exact
/// `(pointer, length)` pair. Empty public values are reported as a null pointer
/// and zero length. On error, both out-parameters are cleared to null and zero
/// and the corresponding status code is returned. Only a null `public_values_ptr` or
/// `public_values_len` leaves both unwritten.
///
/// [`ERE_OK`] only shows that the program ran. The caller compares the public values with the
/// expected bytes and treats a difference as a rejected proof. OpenVM and ZisK return 256 bytes,
/// which are the bytes that the guest wrote followed by zeros, so the caller pads its expected
/// bytes with zeros to 256 bytes. The other kinds return the exact bytes that the guest wrote.
/// [`ERE_ERR_DECODE_PROOF`], [`ERE_ERR_VERIFY`] and [`ERE_ERR_PANIC`] all mean a rejected proof.
///
/// # Safety
///
/// - `handle` must be a live handle returned by [`ere_verifier_new`].
/// - `encoded_proof_ptr` must point to `encoded_proof_len` readable bytes that do not change during
///   the call. It can be null when `encoded_proof_len == 0`. A length above `PTRDIFF_MAX` returns
///   [`ERE_ERR_NULL_PTR`].
/// - `public_values_ptr` must be a non-null, writable `*mut *mut u8`, aligned for a pointer.
/// - `public_values_len` must be a non-null, writable `*mut usize`, aligned for a `uintptr_t`.
/// - The two outputs must not overlap each other, the handle or the proof bytes.
#[unsafe(no_mangle)]
#[must_use = "the status must be checked"]
pub unsafe extern "C" fn ere_verifier_verify(
    handle: *const EreVerifier,
    encoded_proof_ptr: *const u8,
    encoded_proof_len: usize,
    public_values_ptr: *mut *mut u8,
    public_values_len: *mut usize,
) -> i32 {
    guard(|| {
        if public_values_ptr.is_null() || public_values_len.is_null() {
            return ERE_ERR_NULL_PTR;
        }
        unsafe {
            *public_values_ptr = ptr::null_mut();
            *public_values_len = 0;
        }

        if handle.is_null() {
            return ERE_ERR_NULL_PTR;
        }
        let Some(encoded_proof) = (unsafe { as_slice(encoded_proof_ptr, encoded_proof_len) })
        else {
            return ERE_ERR_NULL_PTR;
        };

        let verifier = unsafe { &(*handle).0 };
        let public_values = match verifier.verify(encoded_proof) {
            Ok(public_values) => public_values.0,
            Err(Error::DecodeProof(_)) => return ERE_ERR_DECODE_PROOF,
            Err(Error::Verification(_)) => return ERE_ERR_VERIFY,
            Err(Error::Panic(_)) => return ERE_ERR_PANIC,
            Err(_) => return ERE_ERR_INTERNAL,
        };

        if !public_values.is_empty() {
            let len = public_values.len();
            let ptr = Box::into_raw(public_values.into_boxed_slice()) as *mut u8;
            unsafe {
                *public_values_ptr = ptr;
                *public_values_len = len;
            }
        }
        ERE_OK
    })
}

/// Writes the zkVM kind the verifier was constructed for into `*output` and
/// returns [`ERE_OK`]. On error, `*output` is not written.
///
/// # Safety
///
/// - `handle` must be a live handle returned by [`ere_verifier_new`].
/// - `output` must be a non-null, writable `*mut u32`, aligned for a `uint32_t`, that does not
///   overlap the handle.
#[unsafe(no_mangle)]
#[must_use = "the status must be checked"]
pub unsafe extern "C" fn ere_verifier_zkvm_kind(
    handle: *const EreVerifier,
    output: *mut u32,
) -> i32 {
    guard(|| {
        if handle.is_null() || output.is_null() {
            return ERE_ERR_NULL_PTR;
        }

        let verifier = unsafe { &(*handle).0 };
        unsafe { *output = verifier.zkvm_kind().as_u32() };

        ERE_OK
    })
}

/// Releases a verifier handle. The handle must not be used after this call.
/// Passing a null pointer is a no-op.
///
/// # Safety
///
/// `handle` must be a live handle returned by [`ere_verifier_new`] or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_free(handle: *mut EreVerifier) {
    guard(|| {
        if !handle.is_null() {
            drop(unsafe { Box::from_raw(handle) });
        }
        ERE_OK
    });
}

/// Releases a byte buffer that this library allocated and handed back through a
/// `(pointer, length)` pair. The pair must be exactly what the allocating call
/// produced, and the buffer must not be used after this call. A null pointer or
/// zero length is a no-op.
///
/// # Safety
///
/// `ptr` and `len` must be an unmodified pair previously produced by a function
/// in this library that documents [`ere_bytes_free`] as its release routine and
/// not already freed, or `ptr` must be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_bytes_free(ptr: *mut u8, len: usize) {
    guard(|| {
        if !ptr.is_null() && len != 0 {
            drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(ptr, len)) });
        }
        ERE_OK
    });
}

/// Reconstructs a `&[u8]` from a `(ptr, len)` pair, treating `(NULL, 0)` as
/// the empty slice and returning `None` for `(NULL, len > 0)` and for `len > isize::MAX`.
///
/// # Safety
///
/// When `ptr` is non-null it must point to `len` initialised, readable bytes
/// that are not mutated for the returned slice's lifetime. The caller picks
/// `'a`. It must not outlive the underlying allocation.
unsafe fn as_slice<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        Some(&[])
    } else if ptr.is_null() || len > isize::MAX as usize {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(ptr, len) })
    }
}

/// Runs `body` and returns [`ERE_ERR_PANIC`] when it panics, so no panic unwinds into the caller.
fn guard(body: impl FnOnce() -> i32) -> i32 {
    // Each body clears its outputs before any call that can panic and writes its results last, so a
    // caught panic leaves the outputs of a failure.
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(ERE_ERR_PANIC)
}

#[cfg(test)]
mod tests {
    use crate::{ERE_ERR_PANIC, ERE_OK, guard};

    #[test]
    fn guard_returns_panic_status() {
        assert_eq!(guard(|| panic!("verifier panic")), ERE_ERR_PANIC);
        assert_eq!(guard(|| ERE_OK), ERE_OK);
    }
}

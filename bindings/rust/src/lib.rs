//! Rust binding of `libere_verifier_c`, linked from the directory in `ERE_VERIFIER_LIB_DIR`.

use std::{
    ffi::c_void,
    fmt,
    ptr::{self, NonNull},
    slice,
};

unsafe extern "C" {
    fn ere_verifier_new(kind: u32, vk: *const u8, vk_len: usize, out: *mut *mut c_void) -> i32;
    fn ere_verifier_verify(
        verifier: *const c_void,
        proof: *const u8,
        proof_len: usize,
        out: *mut *mut u8,
        out_len: *mut usize,
    ) -> i32;
    fn ere_verifier_free(verifier: *mut c_void);
    fn ere_bytes_free(bytes: *mut u8, len: usize);
}

/// zkVM of a verifier, with the values of the C API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkVMKind {
    OpenVM = 0,
    SP1 = 1,
    Zisk = 2,
    LambdaVM = 3,
}

/// Status code of a failed call, with the values of the C API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NullPtr = 1,
    BadKind = 2,
    DecodeProgramVk = 3,
    DecodeProof = 4,
    Verify = 5,
    Internal = 6,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ere verifier error: {self:?}")
    }
}

impl std::error::Error for Error {}

fn check(status: i32) -> Result<(), Error> {
    match status {
        0 => Ok(()),
        1 => Err(Error::NullPtr),
        2 => Err(Error::BadKind),
        3 => Err(Error::DecodeProgramVk),
        4 => Err(Error::DecodeProof),
        5 => Err(Error::Verify),
        _ => Err(Error::Internal),
    }
}

/// Verifier bound to a program verifying key.
#[derive(Debug)]
pub struct Verifier(NonNull<c_void>);

// SAFETY: the C API allows concurrent verify calls on one handle, and only drop frees it.
unsafe impl Send for Verifier {}
unsafe impl Sync for Verifier {}

impl Verifier {
    /// Constructs a verifier of `kind` for `program_vk`.
    pub fn new(kind: ZkVMKind, program_vk: &[u8]) -> Result<Self, Error> {
        let mut verifier = ptr::null_mut();
        // SAFETY: the key is readable for its length, and the output is writable.
        check(unsafe {
            ere_verifier_new(
                kind as u32,
                program_vk.as_ptr(),
                program_vk.len(),
                &mut verifier,
            )
        })?;
        NonNull::new(verifier).map(Self).ok_or(Error::Internal)
    }

    /// Verifies `proof` and returns its public values.
    pub fn verify(&self, proof: &[u8]) -> Result<Vec<u8>, Error> {
        let (mut bytes, mut len) = (ptr::null_mut(), 0);
        // SAFETY: the handle is live, the proof is readable for its length, and the outputs are
        // writable.
        check(unsafe {
            ere_verifier_verify(
                self.0.as_ptr(),
                proof.as_ptr(),
                proof.len(),
                &mut bytes,
                &mut len,
            )
        })?;
        if bytes.is_null() {
            return Ok(Vec::new());
        }
        // SAFETY: the library returned `len` readable bytes, freed once after the copy.
        let public_values = unsafe { slice::from_raw_parts(bytes, len) }.to_vec();
        unsafe { ere_bytes_free(bytes, len) };
        Ok(public_values)
    }
}

impl Drop for Verifier {
    fn drop(&mut self) {
        // SAFETY: the handle is live and owned by this value.
        unsafe { ere_verifier_free(self.0.as_ptr()) };
    }
}

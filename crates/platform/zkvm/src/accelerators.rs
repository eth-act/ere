//! Safe API for the [zkvm-standards] accelerators (`zkvm_accelerators.h`).
//!
//! Each function calls the accelerator of the same name, with the standard's byte encodings, and
//! returns [`Error`] when the accelerator reports failure.
//!
//! [zkvm-standards]: https://github.com/eth-act/zkvm-standards

use alloc::{vec, vec::Vec};

/// The accelerator reported failure (`ZKVM_EFAIL`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error;

/// Result of an accelerator.
pub type Result<T> = core::result::Result<T, Error>;

/// `N` bytes aligned to 8: the standard's `zkvm_bytes_N`.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bytes<const N: usize>(pub [u8; N]);

impl<const N: usize> From<[u8; N]> for Bytes<N> {
    fn from(bytes: [u8; N]) -> Self {
        Self(bytes)
    }
}

/// A BN254 G1 point and G2 point, for [`bn254_pairing`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bn254PairingPair {
    pub g1: Bytes<64>,
    pub g2: Bytes<128>,
}

/// A BLS12-381 G1 point and scalar, for [`bls12_g1_msm`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bls12G1MsmPair {
    pub point: Bytes<96>,
    pub scalar: Bytes<32>,
}

/// A BLS12-381 G2 point and scalar, for [`bls12_g2_msm`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bls12G2MsmPair {
    pub point: Bytes<192>,
    pub scalar: Bytes<32>,
}

/// A BLS12-381 G1 point and G2 point, for [`bls12_pairing`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bls12PairingPair {
    pub g1: Bytes<96>,
    pub g2: Bytes<192>,
}

// The layouts of the standard's structs.
const _: () = {
    assert!(align_of::<Bytes<16>>() == 8);
    assert!(size_of::<Bn254PairingPair>() == 192);
    assert!(size_of::<Bls12G1MsmPair>() == 128);
    assert!(size_of::<Bls12G2MsmPair>() == 224);
    assert!(size_of::<Bls12PairingPair>() == 288);
};

/// Keccak-256 of `data`.
pub fn keccak256(data: &[u8]) -> Result<[u8; 32]> {
    output(|out| keccak256_into(data, out))
}

/// [`keccak256`], written to `out`.
pub fn keccak256_into(data: &[u8], out: &mut Bytes<32>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_keccak256(data.as_ptr(), data.len(), out) })
}

/// Whether `sig` (`r || s`) is a valid secp256k1 signature of `msg` by `pubkey` (`x || y`).
pub fn secp256k1_verify(msg: &[u8; 32], sig: &[u8; 64], pubkey: &[u8; 64]) -> Result<bool> {
    verified(|ok| unsafe {
        ffi::zkvm_secp256k1_verify(&Bytes(*msg), &Bytes(*sig), &Bytes(*pubkey), ok)
    })
}

/// The secp256k1 public key (`x || y`) that signed `msg` with `sig` (`r || s`) and `recid`.
pub fn secp256k1_ecrecover(msg: &[u8; 32], sig: &[u8; 64], recid: u8) -> Result<[u8; 64]> {
    output(|out| secp256k1_ecrecover_into(msg, sig, recid, out))
}

/// [`secp256k1_ecrecover`], written to `out`.
pub fn secp256k1_ecrecover_into(
    msg: &[u8; 32],
    sig: &[u8; 64],
    recid: u8,
    out: &mut Bytes<64>,
) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_secp256k1_ecrecover(&Bytes(*msg), &Bytes(*sig), recid, out) })
}

/// SHA-256 of `data`.
pub fn sha256(data: &[u8]) -> Result<[u8; 32]> {
    output(|out| sha256_into(data, out))
}

/// [`sha256`], written to `out`.
pub fn sha256_into(data: &[u8], out: &mut Bytes<32>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_sha256(data.as_ptr(), data.len(), out) })
}

/// RIPEMD-160 of `data`, left-padded with 12 zero bytes.
pub fn ripemd160(data: &[u8]) -> Result<[u8; 32]> {
    output(|out| ripemd160_into(data, out))
}

/// [`ripemd160`], written to `out`.
pub fn ripemd160_into(data: &[u8], out: &mut Bytes<32>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_ripemd160(data.as_ptr(), data.len(), out) })
}

/// `base ^ exp mod modulus`, big-endian, as long as `modulus`.
pub fn modexp(base: &[u8], exp: &[u8], modulus: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![0; modulus.len()];
    modexp_into(base, exp, modulus, &mut out)?;
    Ok(out)
}

/// [`modexp`], written to `out`.
///
/// # Panics
///
/// Panics if `out` is not as long as `modulus`.
pub fn modexp_into(base: &[u8], exp: &[u8], modulus: &[u8], out: &mut [u8]) -> Result<()> {
    assert_eq!(
        out.len(),
        modulus.len(),
        "modexp output must be as long as the modulus"
    );
    Result::from(unsafe {
        ffi::zkvm_modexp(
            base.as_ptr(),
            base.len(),
            exp.as_ptr(),
            exp.len(),
            modulus.as_ptr(),
            modulus.len(),
            out.as_mut_ptr(),
        )
    })
}

/// BN254 G1 `p1 + p2` (EIP-196).
pub fn bn254_g1_add(p1: &[u8; 64], p2: &[u8; 64]) -> Result<[u8; 64]> {
    output(|out| bn254_g1_add_into(p1, p2, out))
}

/// [`bn254_g1_add`], written to `out`.
pub fn bn254_g1_add_into(p1: &[u8; 64], p2: &[u8; 64], out: &mut Bytes<64>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bn254_g1_add(&Bytes(*p1), &Bytes(*p2), out) })
}

/// BN254 G1 `scalar * point` (EIP-196).
pub fn bn254_g1_mul(point: &[u8; 64], scalar: &[u8; 32]) -> Result<[u8; 64]> {
    output(|out| bn254_g1_mul_into(point, scalar, out))
}

/// [`bn254_g1_mul`], written to `out`.
pub fn bn254_g1_mul_into(point: &[u8; 64], scalar: &[u8; 32], out: &mut Bytes<64>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bn254_g1_mul(&Bytes(*point), &Bytes(*scalar), out) })
}

/// Whether the BN254 pairing check of `pairs` holds (EIP-197).
pub fn bn254_pairing(pairs: &[Bn254PairingPair]) -> Result<bool> {
    verified(|ok| unsafe { ffi::zkvm_bn254_pairing(pairs.as_ptr(), pairs.len(), ok) })
}

/// The BLAKE2 compression function F (EIP-152), updating `h` in place.
pub fn blake2f(rounds: u32, h: &mut [u64; 8], m: &[u64; 16], t: &[u64; 2], f: bool) -> Result<()> {
    // On the little-endian targets zkVMs run, `[u64; N]` is the standard's `zkvm_bytes_{8N}`.
    Result::from(unsafe {
        ffi::zkvm_blake2f(
            rounds,
            h.as_mut_ptr().cast(),
            m.as_ptr().cast(),
            t.as_ptr().cast(),
            f.into(),
        )
    })
}

/// Whether `proof` shows that the polynomial committed to by `commitment` is `y` at `z`
/// (EIP-4844).
pub fn kzg_point_eval(
    commitment: &[u8; 48],
    z: &[u8; 32],
    y: &[u8; 32],
    proof: &[u8; 48],
) -> Result<bool> {
    verified(|ok| unsafe {
        ffi::zkvm_kzg_point_eval(
            &Bytes(*commitment),
            &Bytes(*z),
            &Bytes(*y),
            &Bytes(*proof),
            ok,
        )
    })
}

/// BLS12-381 G1 `p1 + p2` (EIP-2537).
pub fn bls12_g1_add(p1: &[u8; 96], p2: &[u8; 96]) -> Result<[u8; 96]> {
    output(|out| bls12_g1_add_into(p1, p2, out))
}

/// [`bls12_g1_add`], written to `out`.
pub fn bls12_g1_add_into(p1: &[u8; 96], p2: &[u8; 96], out: &mut Bytes<96>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bls12_g1_add(&Bytes(*p1), &Bytes(*p2), out) })
}

/// BLS12-381 G1 multi-scalar multiplication of `pairs` (EIP-2537).
pub fn bls12_g1_msm(pairs: &[Bls12G1MsmPair]) -> Result<[u8; 96]> {
    output(|out| bls12_g1_msm_into(pairs, out))
}

/// [`bls12_g1_msm`], written to `out`.
pub fn bls12_g1_msm_into(pairs: &[Bls12G1MsmPair], out: &mut Bytes<96>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bls12_g1_msm(pairs.as_ptr(), pairs.len(), out) })
}

/// BLS12-381 G2 `p1 + p2` (EIP-2537).
pub fn bls12_g2_add(p1: &[u8; 192], p2: &[u8; 192]) -> Result<[u8; 192]> {
    output(|out| bls12_g2_add_into(p1, p2, out))
}

/// [`bls12_g2_add`], written to `out`.
pub fn bls12_g2_add_into(p1: &[u8; 192], p2: &[u8; 192], out: &mut Bytes<192>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bls12_g2_add(&Bytes(*p1), &Bytes(*p2), out) })
}

/// BLS12-381 G2 multi-scalar multiplication of `pairs` (EIP-2537).
pub fn bls12_g2_msm(pairs: &[Bls12G2MsmPair]) -> Result<[u8; 192]> {
    output(|out| bls12_g2_msm_into(pairs, out))
}

/// [`bls12_g2_msm`], written to `out`.
pub fn bls12_g2_msm_into(pairs: &[Bls12G2MsmPair], out: &mut Bytes<192>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bls12_g2_msm(pairs.as_ptr(), pairs.len(), out) })
}

/// Whether the BLS12-381 pairing check of `pairs` holds (EIP-2537).
pub fn bls12_pairing(pairs: &[Bls12PairingPair]) -> Result<bool> {
    verified(|ok| unsafe { ffi::zkvm_bls12_pairing(pairs.as_ptr(), pairs.len(), ok) })
}

/// The BLS12-381 G1 point `fp` maps to (EIP-2537).
pub fn bls12_map_fp_to_g1(fp: &[u8; 48]) -> Result<[u8; 96]> {
    output(|out| bls12_map_fp_to_g1_into(fp, out))
}

/// [`bls12_map_fp_to_g1`], written to `out`.
pub fn bls12_map_fp_to_g1_into(fp: &[u8; 48], out: &mut Bytes<96>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bls12_map_fp_to_g1(&Bytes(*fp), out) })
}

/// The BLS12-381 G2 point `fp2` (`c0 || c1`) maps to (EIP-2537).
pub fn bls12_map_fp2_to_g2(fp2: &[u8; 96]) -> Result<[u8; 192]> {
    output(|out| bls12_map_fp2_to_g2_into(fp2, out))
}

/// [`bls12_map_fp2_to_g2`], written to `out`.
pub fn bls12_map_fp2_to_g2_into(fp2: &[u8; 96], out: &mut Bytes<192>) -> Result<()> {
    Result::from(unsafe { ffi::zkvm_bls12_map_fp2_to_g2(&Bytes(*fp2), out) })
}

/// Whether `sig` (`r || s`) is a valid secp256r1 signature of `msg` by `pubkey` (`x || y`)
/// (EIP-7212).
pub fn secp256r1_verify(msg: &[u8; 32], sig: &[u8; 64], pubkey: &[u8; 64]) -> Result<bool> {
    verified(|ok| unsafe {
        ffi::zkvm_secp256r1_verify(&Bytes(*msg), &Bytes(*sig), &Bytes(*pubkey), ok)
    })
}

fn output<const N: usize>(into: impl FnOnce(&mut Bytes<N>) -> Result<()>) -> Result<[u8; N]> {
    let mut out = Bytes([0; N]);
    into(&mut out)?;
    Ok(out.0)
}

fn verified(call: impl FnOnce(*mut bool) -> ffi::Status) -> Result<bool> {
    let mut ok = false;
    Result::from(call(&mut ok))?;
    Ok(ok)
}

/// The declarations of `zkvm_accelerators.h`.
mod ffi {
    use super::{Bls12G1MsmPair, Bls12G2MsmPair, Bls12PairingPair, Bn254PairingPair, Bytes};

    /// `zkvm_status`: `ZKVM_EOK` (0) or `ZKVM_EFAIL` (-1).
    #[repr(transparent)]
    pub(super) struct Status(core::ffi::c_int);

    impl From<Status> for super::Result<()> {
        fn from(status: Status) -> Self {
            if status.0 == 0 {
                Ok(())
            } else {
                Err(super::Error)
            }
        }
    }

    unsafe extern "C" {
        pub(super) fn zkvm_keccak256(data: *const u8, len: usize, output: *mut Bytes<32>)
        -> Status;
        pub(super) fn zkvm_secp256k1_verify(
            msg: *const Bytes<32>,
            sig: *const Bytes<64>,
            pubkey: *const Bytes<64>,
            verified: *mut bool,
        ) -> Status;
        pub(super) fn zkvm_secp256k1_ecrecover(
            msg: *const Bytes<32>,
            sig: *const Bytes<64>,
            recid: u8,
            output: *mut Bytes<64>,
        ) -> Status;
        pub(super) fn zkvm_sha256(data: *const u8, len: usize, output: *mut Bytes<32>) -> Status;
        pub(super) fn zkvm_ripemd160(data: *const u8, len: usize, output: *mut Bytes<32>)
        -> Status;
        pub(super) fn zkvm_modexp(
            base: *const u8,
            base_len: usize,
            exp: *const u8,
            exp_len: usize,
            modulus: *const u8,
            mod_len: usize,
            output: *mut u8,
        ) -> Status;
        pub(super) fn zkvm_bn254_g1_add(
            p1: *const Bytes<64>,
            p2: *const Bytes<64>,
            result: *mut Bytes<64>,
        ) -> Status;
        pub(super) fn zkvm_bn254_g1_mul(
            point: *const Bytes<64>,
            scalar: *const Bytes<32>,
            result: *mut Bytes<64>,
        ) -> Status;
        pub(super) fn zkvm_bn254_pairing(
            pairs: *const Bn254PairingPair,
            num_pairs: usize,
            verified: *mut bool,
        ) -> Status;
        pub(super) fn zkvm_blake2f(
            rounds: u32,
            h: *mut Bytes<64>,
            m: *const Bytes<128>,
            t: *const Bytes<16>,
            f: u8,
        ) -> Status;
        pub(super) fn zkvm_kzg_point_eval(
            commitment: *const Bytes<48>,
            z: *const Bytes<32>,
            y: *const Bytes<32>,
            proof: *const Bytes<48>,
            verified: *mut bool,
        ) -> Status;
        pub(super) fn zkvm_bls12_g1_add(
            p1: *const Bytes<96>,
            p2: *const Bytes<96>,
            result: *mut Bytes<96>,
        ) -> Status;
        pub(super) fn zkvm_bls12_g1_msm(
            pairs: *const Bls12G1MsmPair,
            num_pairs: usize,
            result: *mut Bytes<96>,
        ) -> Status;
        pub(super) fn zkvm_bls12_g2_add(
            p1: *const Bytes<192>,
            p2: *const Bytes<192>,
            result: *mut Bytes<192>,
        ) -> Status;
        pub(super) fn zkvm_bls12_g2_msm(
            pairs: *const Bls12G2MsmPair,
            num_pairs: usize,
            result: *mut Bytes<192>,
        ) -> Status;
        pub(super) fn zkvm_bls12_pairing(
            pairs: *const Bls12PairingPair,
            num_pairs: usize,
            verified: *mut bool,
        ) -> Status;
        pub(super) fn zkvm_bls12_map_fp_to_g1(
            fp: *const Bytes<48>,
            result: *mut Bytes<96>,
        ) -> Status;
        pub(super) fn zkvm_bls12_map_fp2_to_g2(
            fp2: *const Bytes<96>,
            result: *mut Bytes<192>,
        ) -> Status;
        pub(super) fn zkvm_secp256r1_verify(
            msg: *const Bytes<32>,
            sig: *const Bytes<64>,
            pubkey: *const Bytes<64>,
            verified: *mut bool,
        ) -> Status;
    }
}

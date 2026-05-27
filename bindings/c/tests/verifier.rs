use core::ptr::{NonNull, null, null_mut};

use ere_verifier::zkVMKind;
use ere_verifier_c::{
    ERE_ERR_BAD_KIND, ERE_ERR_DECODE_PROGRAM_VK, ERE_ERR_DECODE_PROOF, ERE_ERR_NULL_PTR,
    ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_LARGE, ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_SMALL, ERE_ERR_VERIFY,
    ERE_OK, EreVerifier, ere_verifier_free, ere_verifier_new, ere_verifier_verify,
    ere_verifier_zkvm_kind,
};

#[derive(Debug)]
struct TestWrapper(*mut EreVerifier);

impl TestWrapper {
    fn new(zkvm_kind: zkVMKind, encoded_program_vk: &[u8]) -> Result<Self, i32> {
        let mut output: *mut EreVerifier = null_mut();
        let rc = unsafe {
            ere_verifier_new(
                zkvm_kind as u32,
                encoded_program_vk.as_ptr(),
                encoded_program_vk.len(),
                &mut output,
            )
        };
        (rc == ERE_OK)
            .then(|| {
                assert!(!output.is_null());
                Self(output)
            })
            .ok_or_else(|| {
                assert!(output.is_null());
                rc
            })
    }

    fn zkvm_kind(&self) -> Result<zkVMKind, i32> {
        let mut output: u32 = 0;
        let rc = unsafe { ere_verifier_zkvm_kind(&*self.0, &mut output) };
        (rc == ERE_OK)
            .then(|| zkVMKind::from_u8(output.try_into().unwrap()).unwrap())
            .ok_or(rc)
    }

    fn verify(&self, encoded_proof: &[u8], public_values_len: usize) -> Result<Vec<u8>, i32> {
        let mut public_values = vec![0u8; public_values_len];
        let rc = unsafe {
            ere_verifier_verify(
                &*self.0,
                encoded_proof.as_ptr(),
                encoded_proof.len(),
                public_values.as_mut_ptr(),
                public_values.len(),
            )
        };
        (rc == ERE_OK).then_some(public_values).ok_or(rc)
    }
}

impl Drop for TestWrapper {
    fn drop(&mut self) {
        unsafe { ere_verifier_free(self.0) };
    }
}

macro_rules! test_verifier {
    ($zkvm_kind:ident) => {
        paste::paste! {
            mod [<$zkvm_kind:lower>] {
                use super::*;

                const PROGRAM_VK: &[u8] = include_bytes!(concat!("../../../crates/verifier/", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/program_vk.bin"));
                const PROOF: &[u8] = include_bytes!(concat!("../../../crates/verifier/", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/proof.bin"));
                const PUBLIC_VALUES: &[u8] = include_bytes!(concat!("../../../crates/verifier/", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/public_values.bin"));

                #[test]
                fn test_verifier() {
                    let verifier = TestWrapper::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();
                    let zkvm_kind = verifier.zkvm_kind().unwrap();
                    let public_values = verifier.verify(PROOF, PUBLIC_VALUES.len()).unwrap();
                    assert_eq!(zkvm_kind, zkVMKind::$zkvm_kind);
                    assert_eq!(&*public_values, PUBLIC_VALUES);
                }

                #[test]
                fn test_invalid_program_vk_decode() {
                    let truncated = &PROGRAM_VK[..PROGRAM_VK.len() - 1];
                    let err = TestWrapper::new(zkVMKind::$zkvm_kind, truncated).unwrap_err();
                    assert_eq!(err, ERE_ERR_DECODE_PROGRAM_VK);

                    let mut extended = PROGRAM_VK.to_vec();
                    extended.push(0xFF);
                    let err = TestWrapper::new(zkVMKind::$zkvm_kind, &extended).unwrap_err();
                    assert_eq!(err, ERE_ERR_DECODE_PROGRAM_VK);
                }

                #[test]
                fn test_invalid_proof_decode() {
                    let verifier = TestWrapper::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();

                    let truncated = &PROOF[..PROOF.len() - 1];
                    let err = verifier.verify(truncated, PUBLIC_VALUES.len()).unwrap_err();
                    assert_eq!(err, ERE_ERR_DECODE_PROOF);

                    let mut extended = PROOF.to_vec();
                    extended.push(0xFF);
                    let err = verifier.verify(&extended, PUBLIC_VALUES.len()).unwrap_err();
                    assert_eq!(err, ERE_ERR_DECODE_PROOF);
                }

                #[test]
                fn test_invalid_proof_verify() {
                    // Proof with byte flipped
                    let verifier = TestWrapper::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();
                    let proof = proof_with_byte_flipped();
                    let err = verifier.verify(&proof, PUBLIC_VALUES.len()).unwrap_err();
                    assert_eq!(err, ERE_ERR_VERIFY);

                    // Unexpected program vk
                    let verifier = verifier_with_unexpected_program_vk();
                    let err = verifier.verify(PROOF, PUBLIC_VALUES.len()).unwrap_err();
                    assert_eq!(err, ERE_ERR_VERIFY);
                }

                fn proof_with_byte_flipped() -> Vec<u8> {
                    let mut proof = PROOF.to_vec();
                    let i = proof.len() / 2;
                    proof[i] ^= 0xFF;
                    proof
                }

                fn verifier_with_unexpected_program_vk() -> TestWrapper {
                    let mut vk = PROGRAM_VK.to_vec();
                    *vk.first_mut().unwrap() ^= 0xFF;
                    TestWrapper::new(zkVMKind::$zkvm_kind, &vk).unwrap()
                }

                #[test]
                fn test_public_values_length_mismatch() {
                    let verifier = TestWrapper::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();
                    let err = verifier.verify(PROOF, 1).unwrap_err();
                    assert_eq!(err, ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_SMALL);
                    let err = verifier.verify(PROOF, 1 << 10).unwrap_err();
                    assert_eq!(err, ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_LARGE);
                }
            }
        }
    };
}

test_verifier!(Airbender);

test_verifier!(OpenVM);

test_verifier!(Risc0);

test_verifier!(SP1);

test_verifier!(Zisk);

#[test]
fn test_null_ptr() {
    for rc in unsafe {
        [
            ere_verifier_new(0, non_null(), 1, null_mut()),
            ere_verifier_verify(null(), null(), 1, null_mut(), 1),
            ere_verifier_verify(non_null(), null(), 1, null_mut(), 1),
            ere_verifier_verify(non_null(), non_null(), 1, null_mut(), 1),
            ere_verifier_zkvm_kind(null(), null_mut()),
            ere_verifier_zkvm_kind(non_null(), null_mut()),
        ]
    } {
        assert_eq!(rc, ERE_ERR_NULL_PTR);
    }
    unsafe { ere_verifier_free(null_mut()) };

    fn non_null<T>() -> *const T {
        NonNull::dangling().as_ptr()
    }
}

#[test]
fn test_invalid_zkvm_kind() {
    let rc = unsafe { ere_verifier_new(99, null(), 0, &mut null_mut()) };
    assert_eq!(rc, ERE_ERR_BAD_KIND);
}

macro_rules! test_verifier {
    ($zkvm_kind:ident) => {
        paste::paste! {
            mod [<$zkvm_kind:lower>] {
                use ere_catalog::zkVMKind;
                use ere_verifier::{Error, Verifier};

                const PROGRAM_VK: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/program_vk.bin"));
                const PROOF: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/proof.bin"));
                const PUBLIC_VALUES: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/public_values.bin"));

                #[test]
                fn test_verifier() {
                    let verifier = Verifier::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();
                    let public_values = verifier.verify(PROOF).unwrap();
                    assert_eq!(&*public_values, PUBLIC_VALUES);
                }

                #[test]
                fn test_invalid_program_vk_decode() {
                    let truncated = &PROGRAM_VK[..PROGRAM_VK.len() - 1];
                    let err = Verifier::new(zkVMKind::$zkvm_kind, truncated).unwrap_err();
                    assert!(matches!(err, Error::DecodeProgramVk(_)));

                    let mut extended = PROGRAM_VK.to_vec();
                    extended.push(0xFF);
                    let err = Verifier::new(zkVMKind::$zkvm_kind, &extended).unwrap_err();
                    assert!(matches!(err, Error::DecodeProgramVk(_)));
                }

                #[test]
                fn test_invalid_proof_decode() {
                    let verifier = Verifier::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();

                    let truncated = &PROOF[..PROOF.len() - 1];
                    let err = verifier.verify(truncated).unwrap_err();
                    assert!(matches!(err, Error::DecodeProof(_)));

                    let mut extended = PROOF.to_vec();
                    extended.push(0xFF);
                    let err = verifier.verify(&extended).unwrap_err();
                    assert!(matches!(err, Error::DecodeProof(_)));
                }

                #[test]
                fn test_invalid_proof_verify() {
                    // Proof with byte flipped
                    let verifier = Verifier::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();
                    let proof = proof_with_byte_flipped();
                    let err = verifier.verify(&proof).unwrap_err();
                    assert!(matches!(err, Error::Verification(_)));

                    // Unexpected program vk
                    let verifier = verifier_with_unexpected_program_vk();
                    let err = verifier.verify(PROOF).unwrap_err();
                    assert!(matches!(err, Error::Verification(_)));
                }

                fn proof_with_byte_flipped() -> Vec<u8> {
                    let mut proof = PROOF.to_vec();
                    let i = proof.len() / 2;
                    proof[i] ^= 0xFF;
                    proof
                }

                fn verifier_with_unexpected_program_vk() -> Verifier {
                    let mut vk = PROGRAM_VK.to_vec();
                    *vk.first_mut().unwrap() ^= 0xFF;
                    Verifier::new(zkVMKind::$zkvm_kind, &vk).unwrap()
                }
            }
        }
    };
}

#[cfg(feature = "nightly")]
test_verifier!(Airbender);

test_verifier!(OpenVM);

test_verifier!(Risc0);

test_verifier!(SP1);

test_verifier!(Zisk);

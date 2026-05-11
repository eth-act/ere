macro_rules! test_verifier {
    ($zkvm_kind:ident) => {
        paste::paste! {
            mod [<$zkvm_kind:lower>] {
                use ere_catalog::zkVMKind;
                use ere_verifier::Verifier;

                #[test]
                fn test_verifier() {
                    const PROGRAM_VK: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/program_vk.bin"));
                    const PROOF: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/proof.bin"));
                    const PUBLIC_VALUES: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/public_values.bin"));

                    let verifier = Verifier::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();
                    let public_values = verifier.verify(PROOF).unwrap();
                    assert_eq!(&*public_values, PUBLIC_VALUES);
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

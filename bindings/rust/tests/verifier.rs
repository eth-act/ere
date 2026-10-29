use std::{fs, thread};

use ere_verifier_rust::{Error, Verifier, ZkVMKind};

fn fixture(zkvm: &str, file: &str) -> Vec<u8> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/verifier");
    fs::read(format!("{dir}/{zkvm}/tests/fixtures/{file}")).unwrap()
}

#[test]
fn verify() {
    for (zkvm, kind) in [
        ("openvm", ZkVMKind::OpenVM),
        ("sp1", ZkVMKind::SP1),
        ("zisk", ZkVMKind::Zisk),
        ("lambdavm", ZkVMKind::LambdaVM),
        ("pico", ZkVMKind::Pico),
    ] {
        let vk = fixture(zkvm, "program_vk.bin");
        let proof = fixture(zkvm, "proof.bin");
        let verifier = Verifier::new(kind, &vk).unwrap();
        thread::scope(|scope| {
            for _ in 0..2 {
                scope.spawn(|| {
                    let public_values = verifier.verify(&proof).unwrap();
                    assert_eq!(public_values, fixture(zkvm, "public_values.bin"));
                });
            }
        });

        let truncated_vk = &vk[..vk.len() - 1];
        assert_eq!(
            Verifier::new(kind, truncated_vk).err(),
            Some(Error::DecodeProgramVk)
        );
        let truncated_proof = &proof[..proof.len() - 1];
        assert_eq!(verifier.verify(truncated_proof), Err(Error::DecodeProof));
        let mut flipped_proof = proof.clone();
        flipped_proof[proof.len() / 2] ^= 0xff;
        assert_eq!(verifier.verify(&flipped_proof), Err(Error::Verify));
    }
}

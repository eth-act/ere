use bincode::error::DecodeError;
use ere_verifier_core::{codec::Decode, zkVMVerifier};
use ere_verifier_risc0::{Error, Risc0ProgramVk, Risc0Proof, Risc0Verifier};
use risc0_zkvm::InnerReceipt;

const PROGRAM_VK: &[u8] = include_bytes!("./fixtures/program_vk.bin");
const PROOF: &[u8] = include_bytes!("./fixtures/proof.bin");
const PUBLIC_VALUES: &[u8] = include_bytes!("./fixtures/public_values.bin");

#[test]
fn test_verifier() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = Risc0Verifier::new(program_vk);
    let proof = Decode::decode_from_slice(PROOF).unwrap();
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

#[test]
fn test_invalid_program_vk_decode() {
    let truncated = &PROGRAM_VK[..PROGRAM_VK.len() - 1];
    let err = Risc0ProgramVk::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 32,
            got: 31
        }
    ));

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = Risc0ProgramVk::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 32,
            got: 33
        }
    ));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    let err = Risc0Proof::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(err, DecodeError::UnexpectedEnd { .. }));

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let err = Risc0Proof::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::Other("trailing bytes after decoded value")
    ));
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = Risc0Verifier::new(program_vk);

    // Unexpected public values
    let proof = proof_with_unexpected_public_values();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::Verify(_)));

    // Invalid merkle proof
    let proof = proof_with_invalid_merkle_path();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::Verify(_)));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = Risc0Proof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::Verify(_)));
}

fn proof_with_unexpected_public_values() -> Risc0Proof {
    let mut proof = Risc0Proof::decode_from_slice(PROOF).unwrap();
    proof.0.journal.bytes[0] ^= 0xFF;
    proof
}

fn proof_with_invalid_merkle_path() -> Risc0Proof {
    let mut proof = Risc0Proof::decode_from_slice(PROOF).unwrap();
    let InnerReceipt::Succinct(ref mut succinct) = proof.0.inner else {
        unreachable!()
    };
    let idx = succinct.seal.len() / 2;
    succinct.seal[idx] = (succinct.seal[idx] >> 1) + 2;
    proof
}

fn verifier_with_unexpected_program_vk() -> Risc0Verifier {
    let mut program_vk = Risc0ProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    program_vk.0.as_mut_words()[0] ^= 0xFF;
    Risc0Verifier::new(program_vk)
}

use bincode::error::DecodeError;
use ere_verifier_core::{codec::Decode, zkVMVerifier};
use ere_verifier_sp1::{Error, SP1ProgramVk, SP1Proof, SP1Verifier};
use sp1_hypercube::PrimeField32;
use sp1_sdk::{SP1Proof as SP1SdkProof, SP1PublicValues};

const PROGRAM_VK: &[u8] = include_bytes!("./fixtures/program_vk.bin");
const PROOF: &[u8] = include_bytes!("./fixtures/proof.bin");
const PUBLIC_VALUES: &[u8] = include_bytes!("./fixtures/public_values.bin");

#[test]
fn test_verifier() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = SP1Verifier::new(program_vk);
    let proof = Decode::decode_from_slice(PROOF).unwrap();
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

#[test]
fn test_invalid_program_vk_decode() {
    let truncated = &PROGRAM_VK[..PROGRAM_VK.len() - 1];
    let err = SP1ProgramVk::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(err, DecodeError::UnexpectedEnd { .. }));

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = SP1ProgramVk::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::Other("trailing bytes after decoded value")
    ));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    let err = SP1Proof::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(err, DecodeError::UnexpectedEnd { .. }));

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let err = SP1Proof::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::Other("trailing bytes after decoded value")
    ));
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = SP1Verifier::new(program_vk);

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
    let proof = SP1Proof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::Verify(_)));
}

fn proof_with_unexpected_public_values() -> SP1Proof {
    let mut proof = SP1Proof::decode_from_slice(PROOF).unwrap();
    let mut bytes = proof.0.public_values.to_vec();
    bytes[0] ^= 0xFF;
    proof.0.public_values = SP1PublicValues::from(&bytes);
    proof
}

fn proof_with_invalid_merkle_path() -> SP1Proof {
    let mut proof = SP1Proof::decode_from_slice(PROOF).unwrap();
    let SP1SdkProof::Compressed(ref mut compress) = proof.0.proof else {
        panic!("expected Compressed proof");
    };
    compress.vk_merkle_proof.path[0][0] = halve_plus_two(compress.vk_merkle_proof.path[0][0]);
    proof
}

fn verifier_with_unexpected_program_vk() -> SP1Verifier {
    let mut program_vk = SP1ProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    program_vk.0.vk.pc_start[0] = halve_plus_two(program_vk.0.vk.pc_start[0]);
    SP1Verifier::new(program_vk)
}

fn halve_plus_two<F: PrimeField32>(value: F) -> F {
    value.halve() + F::two()
}

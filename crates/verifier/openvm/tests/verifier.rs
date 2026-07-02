use core::iter;
use std::io;

use ere_verifier_core::{codec::Decode, zkVMVerifier};
use ere_verifier_openvm::{Error, OpenVMProgramVk, OpenVMProof, OpenVMVerifier};
use openvm_stark_sdk::{
    config::baby_bear_poseidon2::F,
    openvm_stark_backend::p3_field::{PrimeCharacteristicRing, PrimeField32},
};
use openvm_verify_stark_host::error::VerifyStarkError;

const PROGRAM_VK: &[u8] = include_bytes!("./fixtures/program_vk.bin");
const PROOF: &[u8] = include_bytes!("./fixtures/proof.bin");
const PUBLIC_VALUES: &[u8] = include_bytes!("./fixtures/public_values.bin");

#[test]
fn test_verifier() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = OpenVMVerifier::new(program_vk);
    let proof = Decode::decode_from_slice(PROOF).unwrap();
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

#[test]
fn test_invalid_program_vk_decode() {
    let truncated = &PROGRAM_VK[..PROGRAM_VK.len() - 1];
    OpenVMProgramVk::decode_from_slice(truncated).unwrap_err();

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    OpenVMProgramVk::decode_from_slice(&extended).unwrap_err();
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    let Error::DecodeProof(err) = OpenVMProof::decode_from_slice(truncated).unwrap_err() else {
        unreachable!()
    };
    assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let Error::DecodeProof(err) = OpenVMProof::decode_from_slice(&extended).unwrap_err() else {
        unreachable!()
    };
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert_eq!(err.to_string(), "trailing bytes after decoded value");
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = OpenVMVerifier::new(program_vk);

    // Unexpected public values
    let proof = proof_with_unexpected_public_values();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(
        err,
        Error::Verify(VerifyStarkError::UserPvsVerificationFailure(_)),
    ));

    // Invalid merkle proof
    let proof = proof_with_invalid_merkle_path();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(
        err,
        Error::Verify(VerifyStarkError::StarkVerificationFailure(_)),
    ));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(
        err,
        Error::Verify(VerifyStarkError::AppExeCommitMismatch { .. }),
    ));
}

fn proof_with_unexpected_public_values() -> OpenVMProof {
    let mut proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    proof.0.user_pvs_proof.public_values[0] += F::from_u32(1);
    proof
}

fn proof_with_invalid_merkle_path() -> OpenVMProof {
    let mut proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    let value = &mut proof.0.inner.whir_proof.codeword_merkle_proofs[0][0][0][0];
    *value = value.halve() + F::TWO;
    proof
}

fn verifier_with_unexpected_program_vk() -> OpenVMVerifier {
    let mut program_vk = OpenVMProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    program_vk.0.app_exe_commit[0] += F::ONE;
    OpenVMVerifier::new(program_vk)
}

#[test]
fn test_malleable_proof() {
    let bytes = proof_bytes_with_aliased_field_element();
    let Error::DecodeProof(err) = OpenVMProof::decode_from_slice(&bytes).unwrap_err() else {
        unreachable!()
    };
    assert_eq!(err.kind(), io::ErrorKind::Other);
    assert!(err.to_string().contains("F >= F::ORDER_U32"));
}

fn proof_bytes_with_aliased_field_element() -> Vec<u8> {
    const BABYBEAR_MODULUS: u32 = 0x7800_0001;

    let proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    let bytes = iter::empty()
        .chain(&proof.0.inner.whir_proof.codeword_merkle_proofs)
        .flatten()
        .flatten()
        .flatten()
        .map(|value| value.as_canonical_u32().to_le_bytes())
        .find(|bytes| subslice_positions(PROOF, bytes).count() == 1)
        .unwrap();
    let offset = subslice_positions(PROOF, &bytes).next().unwrap();

    let value = u32::from_le_bytes(PROOF[offset..offset + 4].try_into().unwrap());
    let aliased = value.checked_add(BABYBEAR_MODULUS).unwrap();

    let mut proof_aliased = PROOF.to_vec();
    proof_aliased[offset..offset + 4].copy_from_slice(&aliased.to_le_bytes());
    assert_ne!(PROOF, proof_aliased);
    proof_aliased
}

fn subslice_positions(haystack: &[u8], needle: &[u8]) -> impl Iterator<Item = usize> {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(move |(i, subslice)| (subslice == needle).then_some(i))
}

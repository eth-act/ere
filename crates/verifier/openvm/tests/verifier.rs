use core::iter;

use bincode::error::DecodeError;
use ere_verifier_core::{codec::Decode, zkVMVerifier};
use ere_verifier_openvm::{Error, OpenVMProgramVk, OpenVMProof, OpenVMVerifier};
use openvm_circuit::arch::VmVerificationError;
use openvm_continuations::F;
use openvm_stark_sdk::openvm_stark_backend::p3_field::{Field, FieldAlgebra, PrimeField32};

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
    let err = OpenVMProgramVk::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 64,
            got: 63
        }
    ));

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = OpenVMProgramVk::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 64,
            got: 65
        }
    ));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    let Err(err) = OpenVMProof::decode_from_slice(truncated) else {
        unreachable!()
    };
    assert!(matches!(err, DecodeError::UnexpectedEnd { .. }));

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let Err(err) = OpenVMProof::decode_from_slice(&extended) else {
        unreachable!()
    };
    assert!(matches!(
        err,
        DecodeError::Other("trailing bytes after decoded value")
    ));
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
        Error::VmVerification(VmVerificationError::UserPublicValuesError(_))
    ));

    // Invalid merkle proof
    let proof = proof_with_invalid_merkle_path();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::VmVerification(_)));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidAppExeCommit { .. }));
}

fn proof_with_unexpected_public_values() -> OpenVMProof {
    let mut proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    proof.0.user_public_values[0] += F::from_canonical_u32(1);
    proof
}

fn proof_with_invalid_merkle_path() -> OpenVMProof {
    let mut proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    let opening_proof =
        &mut proof.0.inner.opening.proof.query_proofs[0].input_proof[0].opening_proof;
    opening_proof[0][0] = opening_proof[0][0].halve() + F::TWO;
    proof
}

fn verifier_with_unexpected_program_vk() -> OpenVMVerifier {
    let mut program_vk = OpenVMProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    program_vk.0.app_exe_commit.0[0] ^= 0xFF;
    OpenVMVerifier::new(program_vk)
}

#[test]
fn test_malleable_proof() {
    let bytes = proof_bytes_with_aliased_field_element();
    let Err(err) = OpenVMProof::decode_from_slice(&bytes) else {
        unreachable!()
    };
    assert!(matches!(err, DecodeError::OtherString(ref s) if s == "Value is out of range"));
}

fn proof_bytes_with_aliased_field_element() -> Vec<u8> {
    const BABYBEAR_MODULUS: u32 = 0x7800_0001;

    let proof = OpenVMProof::decode_from_slice(PROOF).unwrap();
    let bytes = iter::empty()
        .chain(&proof.0.inner.opening.proof.query_proofs)
        .flat_map(|proof| &proof.input_proof)
        .flat_map(|opening| &opening.opening_proof)
        .flatten()
        .map(|value| value.to_unique_u32().to_le_bytes())
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

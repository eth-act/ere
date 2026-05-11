use bincode::error::DecodeError;
use ere_verifier_core::{codec::Decode, zkVMVerifier};
use ere_verifier_openvm::{Error, OpenVMProgramVk, OpenVMProof, OpenVMVerifier};
use openvm_circuit::arch::VmVerificationError;
use openvm_continuations::F;
use openvm_stark_sdk::openvm_stark_backend::p3_field::{Field, FieldAlgebra};

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

use bincode::error::DecodeError;
use ere_verifier_core::{
    codec::{Decode, Encode},
    zkVMVerifier,
};
use ere_verifier_zisk::{Error, ZiskProgramVk, ZiskProof, ZiskVerifier};
use zisk_verifier::GOLDILOCKS_ORDER;

const PROGRAM_VK: &[u8] = include_bytes!("./fixtures/program_vk.bin");
const PROOF: &[u8] = include_bytes!("./fixtures/proof.bin");
const PUBLIC_VALUES: &[u8] = include_bytes!("./fixtures/public_values.bin");

#[test]
fn test_verifier() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = ZiskVerifier::new(program_vk);
    let proof = Decode::decode_from_slice(PROOF).unwrap();
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

#[test]
fn test_invalid_program_vk_decode() {
    let truncated = &PROGRAM_VK[..PROGRAM_VK.len() - 1];
    let err = ZiskProgramVk::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 32,
            got: 31
        }
    ));

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = ZiskProgramVk::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 32,
            got: 33
        }
    ));

    let mut non_canonical = PROGRAM_VK.to_vec();
    non_canonical[..8].copy_from_slice(&u64::MAX.to_le_bytes());
    let err = ZiskProgramVk::decode_from_slice(&non_canonical).unwrap_err();
    assert!(matches!(err, Error::NonCanonicalProgramVk));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    let err = ZiskProof::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(err, DecodeError::UnexpectedEnd { .. }));

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let err = ZiskProof::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::Other("trailing bytes after decoded value")
    ));

    // Proof above the size limit
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    proof.0.proof.resize(300 * 1024 / 8, 0);
    let err = ZiskProof::decode_from_slice(&proof.encode_to_vec().unwrap()).unwrap_err();
    assert!(matches!(err, DecodeError::LimitExceeded));
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = ZiskVerifier::new(program_vk);

    // Unexpected public values
    let proof = proof_with_unexpected_public_values();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));

    // Invalid merkle proof
    let proof = proof_with_invalid_merkle_path();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));

    // Uncompressed proof
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    proof.0.compressed = false;
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidVadcopFinalProofKind));

    // Unsupported hash family
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    proof.0.hash = "Poseidon2".to_string();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidVadcopFinalProofKind));

    // Public value above u32
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    proof.0.public_values[4] |= 1 << 32;
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidPublicValue));

    // Unexpected public value length
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    proof.0.public_values.push(0);
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidPublicValueLength { .. }));

    // Non-canonical field element
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    proof.0.proof[1] = GOLDILOCKS_ORDER;
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::UnexpectedProgramVk { .. }));
}

fn proof_with_unexpected_public_values() -> ZiskProof {
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    proof.0.public_values[5] ^= 0xFF;
    proof
}

fn proof_with_invalid_merkle_path() -> ZiskProof {
    let mut proof = ZiskProof::decode_from_slice(PROOF).unwrap();
    let idx = proof.0.proof.len() / 2;
    proof.0.proof[idx] = (proof.0.proof[idx] >> 1) + 2;
    proof
}

fn verifier_with_unexpected_program_vk() -> ZiskVerifier {
    let mut program_vk = ZiskProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    program_vk.0[0] ^= 0xFF;
    ZiskVerifier::new(program_vk)
}

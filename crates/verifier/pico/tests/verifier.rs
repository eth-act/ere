use bincode::error::DecodeError;
use ere_verifier_core::{
    codec::{Decode, Encode},
    zkVMVerifier,
};
use ere_verifier_pico::{Error, PicoProgramVk, PicoProof, PicoVerifier};
use pico_verifier::{VerifyError, VmVk};

const PROGRAM_VK: &[u8] = include_bytes!("./fixtures/program_vk.bin");
const PROOF: &[u8] = include_bytes!("./fixtures/proof.bin");
const PUBLIC_VALUES: &[u8] = include_bytes!("./fixtures/public_values.bin");

#[test]
fn test_verifier() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = PicoVerifier::new(program_vk);
    let proof = Decode::decode_from_slice(PROOF).unwrap();
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

#[test]
fn test_codec_roundtrip() {
    let program_vk = PicoProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    assert_eq!(program_vk.encode_to_vec().unwrap(), PROGRAM_VK);
    let proof = PicoProof::decode_from_slice(PROOF).unwrap();
    assert_eq!(proof.encode_to_vec().unwrap(), PROOF);
}

#[test]
fn test_invalid_program_vk_decode() {
    let len = PROGRAM_VK.len();
    for program_vk in [&PROGRAM_VK[..len - 1], &PROGRAM_VK[..12], &[]] {
        let err = PicoProgramVk::decode_from_slice(program_vk).unwrap_err();
        assert!(matches!(err, Error::InvalidProgramVkLength { .. }));
    }

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = PicoProgramVk::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(err, Error::InvalidProgramVkLength { .. }));

    // Limb not less than the KoalaBear modulus
    let err = PicoProgramVk::decode_from_slice(&[0xFF; 100]).unwrap_err();
    assert!(matches!(err, Error::NonCanonicalProgramVk));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    PicoProof::decode_from_slice(truncated).unwrap_err();

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    PicoProof::decode_from_slice(&extended).unwrap_err();

    // Length prefix above the size limit
    let mut oversized = (300 * 1024 + 1u64).to_le_bytes().to_vec();
    oversized.resize(8 + 300 * 1024 + 1, 0);
    let err = PicoProof::decode_from_slice(&oversized).unwrap_err();
    assert!(matches!(err, DecodeError::LimitExceeded));
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = PicoVerifier::new(program_vk);

    // Unexpected public values
    let mut proof = PicoProof::decode_from_slice(PROOF).unwrap();
    proof.committed[0] ^= 0xFF;
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(
        err,
        Error::Verify(VerifyError::CommittedValuesUnbound)
    ));

    // Invalid envelope
    let mut proof = PicoProof::decode_from_slice(PROOF).unwrap();
    let i = proof.envelope.len() / 2;
    proof.envelope[i] ^= 0xFF;
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::Verify(_)));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = PicoProof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(
        err,
        Error::Verify(VerifyError::RiscvVkDigestMismatch { .. })
    ));
}

fn verifier_with_unexpected_program_vk() -> PicoVerifier {
    // Another program: a different entry pc under the same whitelist.
    let program_vk = PicoProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    let mut words = program_vk.0.to_u32s();
    words[8] += 4;
    PicoVerifier::new(PicoProgramVk::new(VmVk::from_u32s(&words).unwrap()))
}

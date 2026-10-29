use ere_verifier_core::{
    codec::{Decode, Encode},
    zkVMVerifier,
};
use ere_verifier_pico::{Error, PicoProgramVk, PicoProof, PicoVerifier};
use pico_verifier::{
    VerifyError, VmVk,
    bundle::{decode_vk_bundle_v2, encode_vk_bundle_v2},
};

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
        assert!(matches!(err, Error::DecodeProgramVk(_)));
    }

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = PicoProgramVk::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(err, Error::DecodeProgramVk(_)));

    // A whitelist root other than the one shipped with Pico.
    let mut bundle = decode_vk_bundle_v2(PROGRAM_VK).unwrap();
    bundle.vk_root[0] = bundle.vk_root[1];
    let bytes = encode_vk_bundle_v2(&bundle.vm_vk, &bundle.vk_root, &bundle.members).unwrap();
    let err = PicoProgramVk::decode_from_slice(&bytes).unwrap_err();
    assert!(matches!(
        err,
        Error::DecodeProgramVk(VerifyError::UntrustedVkRoot)
    ));

    // A membership whose path does not reach the shipped root.
    let mut bundle = decode_vk_bundle_v2(PROGRAM_VK).unwrap();
    bundle.members[0].siblings[0][0] = bundle.members[0].siblings[0][1];
    let bytes = encode_vk_bundle_v2(&bundle.vm_vk, &bundle.vk_root, &bundle.members).unwrap();
    let err = PicoProgramVk::decode_from_slice(&bytes).unwrap_err();
    assert!(matches!(
        err,
        Error::DecodeProgramVk(VerifyError::MemberOffRoot { .. })
    ));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    PicoProof::decode_from_slice(truncated).unwrap_err();

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    PicoProof::decode_from_slice(&extended).unwrap_err();
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
    let bundle = decode_vk_bundle_v2(PROGRAM_VK).unwrap();
    let mut words = bundle.vm_vk.to_u32s();
    words[8] += 4;
    let vm_vk = VmVk::from_u32s(&words).unwrap();
    let bytes = encode_vk_bundle_v2(&vm_vk, &bundle.vk_root, &bundle.members).unwrap();
    PicoVerifier::new(PicoProgramVk::decode_from_slice(&bytes).unwrap())
}

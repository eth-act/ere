use ere_verifier_core::{
    codec::{Decode, Encode},
    zkVMVerifier,
};
use ere_verifier_lambdavm::{Error, LambdaVMProgramVk, LambdaVMProof, LambdaVMVerifier};

const PROGRAM_VK: &[u8] = include_bytes!("./fixtures/program_vk.bin");
const PROOF: &[u8] = include_bytes!("./fixtures/proof.bin");
const PUBLIC_VALUES: &[u8] = include_bytes!("./fixtures/public_values.bin");

#[test]
fn test_verifier() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = LambdaVMVerifier::new(program_vk);
    let proof = Decode::decode_from_slice(PROOF).unwrap();
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

#[test]
fn test_invalid_program_vk_decode() {
    let len = PROGRAM_VK.len();

    let truncated = &PROGRAM_VK[..len - 1];
    let err = LambdaVMProgramVk::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength { expected, got } if expected == len && got == len - 1
    ));

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = LambdaVMProgramVk::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength { expected, got } if expected == len && got == len + 1
    ));

    let err = LambdaVMProgramVk::decode_from_slice(&PROGRAM_VK[..4]).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 8,
            got: 4
        }
    ));

    let mut invalid_elf = PROGRAM_VK.to_vec();
    invalid_elf[8] ^= 0xFF;
    let err = LambdaVMProgramVk::decode_from_slice(&invalid_elf).unwrap_err();
    assert!(matches!(err, Error::DecodeProgramVk(_)));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    let err = LambdaVMProof::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(err, Error::DecodeProof(_)));

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let err = LambdaVMProof::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(err, Error::DecodeProof(_)));
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = LambdaVMVerifier::new(program_vk);

    // Unexpected public values
    let proof = proof_with_unexpected_public_values();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));

    // Invalid STARK proof
    let proof = proof_with_invalid_stark_proof();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = LambdaVMProof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));
}

fn proof_with_unexpected_public_values() -> LambdaVMProof {
    let mut proof = LambdaVMProof::decode_from_slice(PROOF).unwrap();
    proof.0.public_output[0] ^= 0xFF;
    proof
}

fn proof_with_invalid_stark_proof() -> LambdaVMProof {
    let mut bytes = PROOF.to_vec();
    let i = bytes.len() / 2;
    bytes[i] ^= 0xFF;
    LambdaVMProof::decode_from_slice(&bytes).unwrap()
}

fn verifier_with_unexpected_program_vk() -> LambdaVMVerifier {
    // Flip a byte in the middle of the ELF, which keeps it loadable.
    let mut program_vk = LambdaVMProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    let i = program_vk.0.len() / 2;
    program_vk.0[i] ^= 0xFF;
    LambdaVMVerifier::new(program_vk)
}

// FIXME: Do we need to restrict proof to be non-malleable?
#[test]
fn test_malleable_proof() {
    let bytes = proof_bytes_with_aliased_field_element();
    let proof = LambdaVMProof::decode_from_slice(&bytes).unwrap();
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = LambdaVMVerifier::new(program_vk);
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

fn proof_bytes_with_aliased_field_element() -> Vec<u8> {
    const GOLDILOCKS_MODULUS: u64 = 0xFFFF_FFFF_0000_0001;
    const MARKER: u64 = 0x1234_5678_9ABC_DEF1;

    // Small trace values repeat all over the proof, so locate one by encoding a
    // marker in its place.
    let mut proof = LambdaVMProof::decode_from_slice(PROOF).unwrap();
    let value = proof
        .0
        .proof
        .proofs
        .iter_mut()
        .flat_map(|proof| &mut proof.deep_poly_openings)
        .flat_map(|opening| &mut opening.main_trace_polys.evaluations)
        .find(|value| value.value().checked_add(GOLDILOCKS_MODULUS).is_some())
        .unwrap();
    *value = MARKER.into();
    let marked = proof.encode_to_vec().unwrap();
    let offset = subslice_positions(&marked, &MARKER.to_le_bytes())
        .next()
        .unwrap();

    let value = u64::from_le_bytes(PROOF[offset..offset + 8].try_into().unwrap());
    let aliased = value.checked_add(GOLDILOCKS_MODULUS).unwrap();

    let mut proof_aliased = PROOF.to_vec();
    proof_aliased[offset..offset + 8].copy_from_slice(&aliased.to_le_bytes());
    assert_ne!(PROOF, proof_aliased);
    proof_aliased
}

fn subslice_positions(haystack: &[u8], needle: &[u8]) -> impl Iterator<Item = usize> {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(move |(i, subslice)| (subslice == needle).then_some(i))
}

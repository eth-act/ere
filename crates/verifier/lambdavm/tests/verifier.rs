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
    assert!(matches!(err, Error::InvalidProgramVkElf(_)));
}

#[test]
fn test_invalid_proof_decode() {
    let truncated = &PROOF[..PROOF.len() - 1];
    let err = LambdaVMProof::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(err, Error::Deserialize(_)));

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let err = LambdaVMProof::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(err, Error::Deserialize(_)));
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = LambdaVMVerifier::new(program_vk);

    // Unexpected public values
    let proof = proof_with_unexpected_public_values();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof | Error::Verify(_)));

    // Invalid STARK proof
    let proof = proof_with_byte_flipped();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof | Error::Verify(_)));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = LambdaVMProof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof | Error::Verify(_)));
}

fn proof_with_unexpected_public_values() -> LambdaVMProof {
    let mut proof = LambdaVMProof::decode_from_slice(PROOF).unwrap();
    proof.0.public_output[0] ^= 0xFF;
    proof
}

fn proof_with_byte_flipped() -> LambdaVMProof {
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

/// Adds the Goldilocks modulus to a main trace opening small enough that the sum fits a `u64`.
fn proof_bytes_with_aliased_field_element() -> Vec<u8> {
    const GOLDILOCKS_MODULUS: u64 = 0xFFFF_FFFF_0000_0001;
    const MARKER: u64 = 0x1234_5678_9ABC_DEF1;

    let proof = LambdaVMProof::decode_from_slice(PROOF).unwrap();
    let (table, query, column, value) = proof
        .0
        .proof
        .proofs
        .iter()
        .enumerate()
        .find_map(|(table, proof)| {
            proof
                .deep_poly_openings
                .iter()
                .enumerate()
                .find_map(|(query, opening)| {
                    opening
                        .main_trace_polys
                        .evaluations
                        .iter()
                        .enumerate()
                        .find_map(|(column, value)| {
                            let value = *value.value();
                            (value < u64::MAX - GOLDILOCKS_MODULUS)
                                .then_some((table, query, column, value))
                        })
                })
        })
        .unwrap();

    // Find the offset of the value by encoding a proof that holds a marker in its place.
    let mut marked = proof.clone();
    marked.0.proof.proofs[table].deep_poly_openings[query]
        .main_trace_polys
        .evaluations[column] = MARKER.into();
    let marked = marked.encode_to_vec().unwrap();
    let marker = MARKER.to_le_bytes();
    let mut offsets = subslice_positions(&marked, &marker);
    let offset = offsets.next().unwrap();
    assert!(offsets.next().is_none());
    assert_eq!(PROOF[offset..offset + 8], value.to_le_bytes());

    let mut proof_aliased = PROOF.to_vec();
    proof_aliased[offset..offset + 8].copy_from_slice(&(value + GOLDILOCKS_MODULUS).to_le_bytes());
    assert_ne!(PROOF, proof_aliased);
    proof_aliased
}

fn subslice_positions(haystack: &[u8], needle: &[u8]) -> impl Iterator<Item = usize> {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(move |(i, subslice)| (subslice == needle).then_some(i))
}

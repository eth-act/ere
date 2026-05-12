use bincode::error::DecodeError;
use ere_verifier_airbender::{AirbenderProgramVk, AirbenderProof, AirbenderVerifier, Error};
use ere_verifier_core::{codec::Decode, zkVMVerifier};

const PROGRAM_VK: &[u8] = include_bytes!("./fixtures/program_vk.bin");
const PROOF: &[u8] = include_bytes!("./fixtures/proof.bin");
const PUBLIC_VALUES: &[u8] = include_bytes!("./fixtures/public_values.bin");

#[test]
fn test_verifier() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = AirbenderVerifier::new(program_vk);
    let proof = Decode::decode_from_slice(PROOF).unwrap();
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

#[test]
fn test_invalid_program_vk_decode() {
    let truncated = &PROGRAM_VK[..PROGRAM_VK.len() - 1];
    let err = AirbenderProgramVk::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProgramVkLength {
            expected: 32,
            got: 31
        }
    ));

    let mut extended = PROGRAM_VK.to_vec();
    extended.push(0xFF);
    let err = AirbenderProgramVk::decode_from_slice(&extended).unwrap_err();
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
    let err = AirbenderProof::decode_from_slice(truncated).unwrap_err();
    assert!(matches!(err, DecodeError::UnexpectedEnd { .. }));

    let mut extended = PROOF.to_vec();
    extended.push(0xFF);
    let err = AirbenderProof::decode_from_slice(&extended).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::Other("trailing bytes after decoded value")
    ));
}

#[test]
fn test_invalid_proof_verify() {
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = AirbenderVerifier::new(program_vk);

    // Unexpected public values
    let proof = proof_with_unexpected_public_values();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));

    // Invalid merkle proof
    let proof = proof_with_invalid_merkle_path();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::InvalidProof));

    // Unexpected program vk
    let verifier = verifier_with_unexpected_program_vk();
    let proof = AirbenderProof::decode_from_slice(PROOF).unwrap();
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(err, Error::UnexpectedHashChain { .. }));
}

fn proof_with_unexpected_public_values() -> AirbenderProof {
    let mut proof = AirbenderProof::decode_from_slice(PROOF).unwrap();
    proof.0.register_final_values[0].value ^= 0xFF;
    proof
}

fn proof_with_invalid_merkle_path() -> AirbenderProof {
    let mut proof = AirbenderProof::decode_from_slice(PROOF).unwrap();
    let family_proofs = proof.0.circuit_families_proofs.values_mut().next().unwrap();
    let merkle_proof = &mut family_proofs[0].queries[0].initial_fri_query.merkle_proof;
    merkle_proof[0][0] = (merkle_proof[0][0] >> 1) + 2;
    proof
}

fn verifier_with_unexpected_program_vk() -> AirbenderVerifier {
    let mut program_vk = AirbenderProgramVk::decode_from_slice(PROGRAM_VK).unwrap();
    program_vk.0[0] ^= 0xFF;
    AirbenderVerifier::new(program_vk)
}

// FIXME: Do we need to restrict proof to be non-malleable?
#[test]
fn test_malleable_proof() {
    let bytes = proof_bytes_with_aliased_field_element();
    let proof = AirbenderProof::decode_from_slice(&bytes).unwrap();
    let program_vk = Decode::decode_from_slice(PROGRAM_VK).unwrap();
    let verifier = AirbenderVerifier::new(program_vk);
    let public_values = verifier.verify(&proof).unwrap();
    assert_eq!(&*public_values, PUBLIC_VALUES);
}

fn proof_bytes_with_aliased_field_element() -> Vec<u8> {
    const MERSENNE31_MODULUS: u32 = 0x7FFF_FFFF;

    let proof = AirbenderProof::decode_from_slice(PROOF).unwrap();
    let bytes = proof
        .0
        .circuit_families_proofs
        .values()
        .flatten()
        .flat_map(|proof| &proof.evaluations_at_random_points)
        .flat_map(|point| [&point.c0.c0, &point.c0.c1, &point.c1.c0, &point.c1.c1])
        .map(|value| value.0.to_le_bytes())
        .find(|bytes| subslice_positions(PROOF, bytes).count() == 1)
        .unwrap();
    let offset = subslice_positions(PROOF, &bytes).next().unwrap();

    let value = u32::from_le_bytes(PROOF[offset..offset + 4].try_into().unwrap());
    let aliased = value.checked_add(MERSENNE31_MODULUS).unwrap();

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

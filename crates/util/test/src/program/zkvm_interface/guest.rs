//! Guest-side call into the zkVM accelerator symbols for one recorded vector.
//!
//! Each arm mirrors the `revm::precompile::Crypto` implementation a stateless validator guest
//! ships. A recorded outcome is therefore directly comparable.

use alloc::{vec, vec::Vec};

use zkvm_interface::{
    zkvm_blake2f_message, zkvm_blake2f_offset, zkvm_blake2f_state, zkvm_bls12_381_fp,
    zkvm_bls12_381_fp2, zkvm_bls12_381_g1_msm_pair, zkvm_bls12_381_g1_point,
    zkvm_bls12_381_g2_msm_pair, zkvm_bls12_381_g2_point, zkvm_bls12_381_pairing_pair,
    zkvm_bn254_g1_point, zkvm_bn254_pairing_pair, zkvm_bn254_scalar, zkvm_keccak256_hash,
    zkvm_kzg_commitment, zkvm_kzg_field_element, zkvm_kzg_proof, zkvm_ripemd160_hash,
    zkvm_secp256k1_hash, zkvm_secp256k1_pubkey, zkvm_secp256k1_signature, zkvm_secp256r1_hash,
    zkvm_secp256r1_pubkey, zkvm_secp256r1_signature, zkvm_sha256_hash,
};

use crate::program::zkvm_interface::{Accelerator, Outcome, Vector};

/// Runs `vector` through the accelerator and returns what it produced.
pub(crate) fn run(vector: &Vector) -> Outcome {
    let inputs = &vector.inputs;
    match vector.accelerator {
        Accelerator::Sha256 => {
            let mut output = zkvm_sha256_hash { data: [0; 32] };
            let status = unsafe {
                zkvm_interface::zkvm_sha256(inputs[0].as_ptr(), inputs[0].len(), &mut output)
            };
            Outcome::new(status, &output.data)
        }
        Accelerator::Ripemd160 => {
            let mut output = zkvm_ripemd160_hash { data: [0; 32] };
            let status = unsafe {
                zkvm_interface::zkvm_ripemd160(inputs[0].as_ptr(), inputs[0].len(), &mut output)
            };
            Outcome::new(status, &output.data)
        }
        Accelerator::Bn254G1Add => {
            let p1 = zkvm_bn254_g1_point {
                data: to_array(&inputs[0]),
            };
            let p2 = zkvm_bn254_g1_point {
                data: to_array(&inputs[1]),
            };
            let mut result = zkvm_bn254_g1_point { data: [0; 64] };
            let status = unsafe { zkvm_interface::zkvm_bn254_g1_add(&p1, &p2, &mut result) };
            Outcome::new(status, &result.data)
        }
        Accelerator::Bn254G1Mul => {
            let point = zkvm_bn254_g1_point {
                data: to_array(&inputs[0]),
            };
            let scalar = zkvm_bn254_scalar {
                data: to_array(&inputs[1]),
            };
            let mut result = zkvm_bn254_g1_point { data: [0; 64] };
            let status = unsafe { zkvm_interface::zkvm_bn254_g1_mul(&point, &scalar, &mut result) };
            Outcome::new(status, &result.data)
        }
        Accelerator::Bn254PairingCheck => {
            let pairs = to_aligned_pairs(vector);
            let mut verified = false;
            let status = unsafe {
                zkvm_interface::zkvm_bn254_pairing(
                    pairs.as_ptr() as *const zkvm_bn254_pairing_pair,
                    vector.num_pairs(),
                    &mut verified,
                )
            };
            Outcome::new(status, &[u8::from(verified)])
        }
        Accelerator::Secp256k1Ecrecover => {
            let sig = zkvm_secp256k1_signature {
                data: to_array(&inputs[0]),
            };
            let msg = zkvm_secp256k1_hash {
                data: to_array(&inputs[1]),
            };
            let recid = to_byte(&inputs[2]);
            let mut pubkey = zkvm_secp256k1_pubkey { data: [0; 64] };
            let status =
                unsafe { zkvm_interface::zkvm_secp256k1_ecrecover(&msg, &sig, recid, &mut pubkey) };
            // The reference implementation returns the address the public key hashes to,
            // left-padded.
            let mut hash = keccak256(&pubkey.data);
            hash[..12].fill(0);
            Outcome::new(status, &hash)
        }
        Accelerator::Modexp => {
            let (base, exp, modulus) = (&inputs[0], &inputs[1], &inputs[2]);
            let mut output = vec![0u8; modulus.len()];
            let status = unsafe {
                zkvm_interface::zkvm_modexp(
                    base.as_ptr(),
                    base.len(),
                    exp.as_ptr(),
                    exp.len(),
                    modulus.as_ptr(),
                    modulus.len(),
                    output.as_mut_ptr(),
                )
            };
            Outcome::new(status, output)
        }
        Accelerator::Blake2Compress => {
            let mut state = zkvm_blake2f_state {
                data: to_array(&inputs[0]),
            };
            let message = zkvm_blake2f_message {
                data: to_array(&inputs[1]),
            };
            let offset = zkvm_blake2f_offset {
                data: to_array(&inputs[2]),
            };
            let rounds = u32::from_le_bytes(to_array(&inputs[3]));
            let final_block = to_byte(&inputs[4]);
            let status = unsafe {
                zkvm_interface::zkvm_blake2f(rounds, &mut state, &message, &offset, final_block)
            };
            Outcome::new(status, &state.data)
        }
        Accelerator::Secp256r1VerifySignature => {
            let msg = zkvm_secp256r1_hash {
                data: to_array(&inputs[0]),
            };
            let sig = zkvm_secp256r1_signature {
                data: to_array(&inputs[1]),
            };
            let pubkey = zkvm_secp256r1_pubkey {
                data: to_array(&inputs[2]),
            };
            let mut verified = false;
            let status = unsafe {
                zkvm_interface::zkvm_secp256r1_verify(&msg, &sig, &pubkey, &mut verified)
            };
            // The reference implementation returns one boolean, so a failed status folds into it.
            Outcome::new(0, &[u8::from(status == 0 && verified)])
        }
        Accelerator::VerifyKzgProof => {
            let z = zkvm_kzg_field_element {
                data: to_array(&inputs[0]),
            };
            let y = zkvm_kzg_field_element {
                data: to_array(&inputs[1]),
            };
            let commitment = zkvm_kzg_commitment {
                data: to_array(&inputs[2]),
            };
            let proof = zkvm_kzg_proof {
                data: to_array(&inputs[3]),
            };
            let mut verified = false;
            let status = unsafe {
                zkvm_interface::zkvm_kzg_point_eval(&commitment, &z, &y, &proof, &mut verified)
            };
            Outcome::new(0, &[u8::from(status == 0 && verified)])
        }
        Accelerator::Bls12381G1Add => {
            let p1 = zkvm_bls12_381_g1_point {
                data: to_array(&inputs[0]),
            };
            let p2 = zkvm_bls12_381_g1_point {
                data: to_array(&inputs[1]),
            };
            let mut result = zkvm_bls12_381_g1_point { data: [0; 96] };
            let status = unsafe { zkvm_interface::zkvm_bls12_g1_add(&p1, &p2, &mut result) };
            Outcome::new(status, &result.data)
        }
        Accelerator::Bls12381G1Msm => {
            let pairs = to_aligned_pairs(vector);
            let mut result = zkvm_bls12_381_g1_point { data: [0; 96] };
            let status = unsafe {
                zkvm_interface::zkvm_bls12_g1_msm(
                    pairs.as_ptr() as *const zkvm_bls12_381_g1_msm_pair,
                    vector.num_pairs(),
                    &mut result,
                )
            };
            Outcome::new(status, &result.data)
        }
        Accelerator::Bls12381G2Add => {
            let p1 = zkvm_bls12_381_g2_point {
                data: to_array(&inputs[0]),
            };
            let p2 = zkvm_bls12_381_g2_point {
                data: to_array(&inputs[1]),
            };
            let mut result = zkvm_bls12_381_g2_point { data: [0; 192] };
            let status = unsafe { zkvm_interface::zkvm_bls12_g2_add(&p1, &p2, &mut result) };
            Outcome::new(status, &result.data)
        }
        Accelerator::Bls12381G2Msm => {
            let pairs = to_aligned_pairs(vector);
            let mut result = zkvm_bls12_381_g2_point { data: [0; 192] };
            let status = unsafe {
                zkvm_interface::zkvm_bls12_g2_msm(
                    pairs.as_ptr() as *const zkvm_bls12_381_g2_msm_pair,
                    vector.num_pairs(),
                    &mut result,
                )
            };
            Outcome::new(status, &result.data)
        }
        Accelerator::Bls12381PairingCheck => {
            let pairs = to_aligned_pairs(vector);
            let mut verified = false;
            let status = unsafe {
                zkvm_interface::zkvm_bls12_pairing(
                    pairs.as_ptr() as *const zkvm_bls12_381_pairing_pair,
                    vector.num_pairs(),
                    &mut verified,
                )
            };
            Outcome::new(status, &[u8::from(verified)])
        }
        Accelerator::Bls12381FpToG1 => {
            let field_element = zkvm_bls12_381_fp {
                data: to_array(&inputs[0]),
            };
            let mut result = zkvm_bls12_381_g1_point { data: [0; 96] };
            let status =
                unsafe { zkvm_interface::zkvm_bls12_map_fp_to_g1(&field_element, &mut result) };
            Outcome::new(status, &result.data)
        }
        Accelerator::Bls12381Fp2ToG2 => {
            let field_element = zkvm_bls12_381_fp2 {
                data: to_array(&inputs[0]),
            };
            let mut result = zkvm_bls12_381_g2_point { data: [0; 192] };
            let status =
                unsafe { zkvm_interface::zkvm_bls12_map_fp2_to_g2(&field_element, &mut result) };
            Outcome::new(status, &result.data)
        }
    }
}

fn to_array<const N: usize>(bytes: &[u8]) -> [u8; N] {
    bytes.try_into().expect("argument has the declared length")
}

fn to_byte(bytes: &[u8]) -> u8 {
    to_array::<1>(bytes)[0]
}

/// The packed pair array of `vector`, copied into an 8-aligned buffer. Every accelerator struct is
/// `align(8)`, so the byte vector cannot be cast in place.
fn to_aligned_pairs(vector: &Vector) -> Vec<u64> {
    let bytes = &vector.inputs[0];
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| u64::from_le_bytes(*chunk))
        .collect()
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut output = zkvm_keccak256_hash { data: [0; 32] };
    let status = unsafe { zkvm_interface::zkvm_keccak256(data.as_ptr(), data.len(), &mut output) };
    assert_eq!(status, 0, "keccak256 failed");
    output.data
}

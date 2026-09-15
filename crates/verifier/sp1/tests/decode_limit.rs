//! A length prefix in a proof must not decide how much the decoder allocates.

use ere_verifier_core::codec::Decode;
use ere_verifier_sp1::SP1Proof;

/// A proof whose declared length the input does not carry is an error, not an allocation of that
/// size.
///
/// The twelve bytes below are a selector this decoder accepts followed by a length near
/// 2.1e17. Before the decode was bounded they aborted the process: the length is representable,
/// so it reached the allocator, the allocation failed, and an allocation failure calls `abort()`.
/// That is not a panic, so no caller could catch it. Both fields are attacker-chosen wherever
/// proof bytes arrive from a network.
#[test]
fn a_declared_length_beyond_the_input_is_rejected() {
    let mut input = 3u32.to_le_bytes().to_vec();
    input.extend_from_slice(&211_946_530_762_463_256u64.to_le_bytes());

    let error = SP1Proof::decode_from_slice(&input).expect_err("decode must fail");
    assert!(
        matches!(error, bincode::error::DecodeError::LimitExceeded),
        "expected the bound to reject the declared length, got {error:?}"
    );
}

/// Truncated and arbitrary inputs stay rejected, and still without allocating on their say-so.
#[test]
fn malformed_input_is_rejected() {
    for input in [vec![0u8; 1], vec![0xAAu8; 4096], Vec::new()] {
        assert!(SP1Proof::decode_from_slice(&input).is_err());
    }
}

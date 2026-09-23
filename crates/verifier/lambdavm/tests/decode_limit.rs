//! The size of a proof must not decide how much the decoder allocates.

use ere_verifier_core::codec::{Decode, MAX_DECODE_BYTES};
use ere_verifier_lambdavm::{Error, LambdaVMProof};

/// A proof longer than the decode limit is rejected before it is copied or validated.
#[test]
fn an_input_beyond_the_limit_is_rejected() {
    let input = vec![0u8; MAX_DECODE_BYTES + 1];

    let error = LambdaVMProof::decode_from_slice(&input).expect_err("decode must fail");
    assert!(
        matches!(error, Error::DecodeLimitExceeded { limit: MAX_DECODE_BYTES, got } if got == MAX_DECODE_BYTES + 1),
        "expected the bound to reject the input, got {error:?}"
    );
}

/// Truncated and arbitrary inputs stay rejected.
#[test]
fn malformed_input_is_rejected() {
    for input in [vec![0u8; 1], vec![0xAAu8; 4096], Vec::new()] {
        assert!(LambdaVMProof::decode_from_slice(&input).is_err());
    }
}

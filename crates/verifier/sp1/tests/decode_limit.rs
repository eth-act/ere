//! Demonstration only. This test aborts the process on this branch.
//!
//! `impl_codec_by_bincode_legacy!` decodes with `bincode::config::legacy()`, which carries no
//! limit, so a length prefix in the input decides how much a container allocates and the
//! allocation happens before any of that length has been read.

use ere_verifier_core::codec::Decode;
use ere_verifier_sp1::SP1Proof;

/// Twelve bytes abort the process.
///
/// Run with `cargo test -p ere-verifier-sp1 --test decode_limit`. The result is
///
/// ```text
/// memory allocation of 211946530762463256 bytes failed
/// error: test failed ... (signal: 6, SIGABRT: process abort signal)
/// ```
///
/// The length is representable, so it is not caught as a capacity overflow. It becomes a real
/// allocation request, the allocator fails, and an allocation failure calls `abort()`. That is
/// not a panic, so the assertion below is never reached and no caller could catch it either.
///
/// Both fields are chosen by whoever supplies the bytes. The selector is the one an OpenVM
/// stateless-validator proof begins with, which `ProofFromNetwork` also accepts, and the
/// `UnexpectedProofKind` check that would reject this input runs on the decoded value, so it
/// never gets to run.
#[test]
fn a_declared_length_beyond_the_input_aborts_the_process() {
    let mut input = 3u32.to_le_bytes().to_vec();
    input.extend_from_slice(&211_946_530_762_463_256u64.to_le_bytes());

    let error = SP1Proof::decode_from_slice(&input).expect_err("decode must fail");
    assert!(
        matches!(error, bincode::error::DecodeError::LimitExceeded),
        "expected the declared length to be rejected, got {error:?}"
    );
}

/// For contrast, and to show the abort is not simply "malformed input crashes it". Arbitrary and
/// truncated bytes are already rejected cleanly, because their leading word is not a selector
/// this decoder accepts, so no length is ever read. This test passes on this branch.
#[test]
fn malformed_input_is_rejected() {
    for input in [vec![0u8; 1], vec![0xAAu8; 4096], Vec::new()] {
        assert!(SP1Proof::decode_from_slice(&input).is_err());
    }
}

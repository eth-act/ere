//! Decoding accepts values within the bound and rejects lengths that exceed it.

use ere_codec::{Decode, Encode, MAX_DECODE_BYTES, impl_codec_by_bincode_legacy};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Message {
    Empty,
    Body(Vec<u32>),
}

impl_codec_by_bincode_legacy!(Message);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Strict(Vec<u32>);

impl_codec_by_bincode_legacy!(Strict, reject_trailing_bytes);

#[test]
fn values_within_the_bound_round_trip() {
    for message in [Message::Empty, Message::Body(vec![7u32; 4096])] {
        let encoded = message.encode_to_vec().expect("encodes");
        assert_eq!(
            Message::decode_from_slice(&encoded).expect("decodes"),
            message
        );
    }

    let strict = Strict(vec![3u32; 1024]);
    let encoded = strict.encode_to_vec().expect("encodes");
    assert_eq!(
        Strict::decode_from_slice(&encoded).expect("decodes"),
        strict
    );
}

#[test]
fn values_exceeding_the_bound_are_rejected() {
    let values = vec![0u32; MAX_DECODE_BYTES / size_of::<u32>() + 1];
    let strict_input = Strict(values.clone()).encode_to_vec().expect("encodes");
    let message_input = Message::Body(values).encode_to_vec().expect("encodes");

    for error in [
        Message::decode_from_slice(&message_input).expect_err("decode must fail"),
        Strict::decode_from_slice(&strict_input).expect_err("decode must fail"),
    ] {
        assert!(
            matches!(error, bincode::error::DecodeError::LimitExceeded),
            "expected the bound to reject the oversized value, got {error:?}"
        );
    }
}

#[test]
fn trailing_bytes_are_still_rejected() {
    let mut encoded = Strict(vec![1u32, 2, 3]).encode_to_vec().expect("encodes");
    encoded.push(0);
    assert!(Strict::decode_from_slice(&encoded).is_err());
}

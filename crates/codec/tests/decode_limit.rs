//! The decode bound must not narrow what these implementations accept.

use ere_codec::{Decode, Encode, impl_codec_by_bincode_legacy};
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
fn trailing_bytes_are_still_rejected() {
    let mut encoded = Strict(vec![1u32, 2, 3]).encode_to_vec().expect("encodes");
    encoded.push(0);
    assert!(Strict::decode_from_slice(&encoded).is_err());
}

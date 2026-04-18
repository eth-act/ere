use core::fmt::Debug;

use ere_codec::{Decode, Encode};
use ere_platform_core::Platform;

pub mod basic;

/// Program that can be run given [`Platform`] implementation.
pub trait Program {
    type Input: Encode + Decode + Clone + Debug + Send + Sync;
    type Output: Encode + Decode + Clone + Debug + Send + Sync + PartialEq;

    fn compute(input: Self::Input) -> Self::Output;

    fn run<P: Platform>() {
        let input_bytes = P::read_whole_input();
        let input = Self::Input::decode_from_slice(&input_bytes).unwrap();
        let output = Self::compute(input);
        let output_bytes = output.encode_to_vec().unwrap();
        P::write_whole_output(&output_bytes);
    }
}

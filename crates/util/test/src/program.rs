use alloc::vec::Vec;
use core::{convert::identity, fmt::Debug};

use ere_codec::{Decode, Encode};
use ere_platform_core::Platform;
use sha2::{Digest, Sha256};

pub mod basic;

/// Program that can be run given [`Platform`] implementation.
pub trait Program {
    type Input: Encode + Decode + Clone + Debug + Send + Sync;
    type Output: Encode + Decode + Clone + Debug + Send + Sync + PartialEq;

    fn compute(input: Self::Input) -> Self::Output;

    fn run<P: Platform>()
    where
        Self: Sized,
    {
        run_inner::<Self, P, _>(identity);
    }

    fn run_output_sha256<P: Platform>()
    where
        Self: Sized,
    {
        run_inner::<Self, P, _>(|output_bytes| Sha256::digest(&output_bytes));
    }
}

fn run_inner<G: Program, P: Platform, T: AsRef<[u8]>>(
    output_bytes_modifier: impl Fn(Vec<u8>) -> T,
) {
    let input_bytes = P::read_whole_input();
    let input = G::Input::decode_from_slice(&input_bytes).unwrap();
    let output = G::compute(input);
    let output_bytes = output.encode_to_vec().unwrap();
    let modified_output_bytes = output_bytes_modifier(output_bytes);
    P::write_whole_output(modified_output_bytes.as_ref());
}

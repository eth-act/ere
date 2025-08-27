#![no_main]

use test_utils::guest::{BasicStruct, BASIC_PROGRAM_BYTES_LENGTH};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    // Read `bytes`.
    let bytes = sp1_zkvm::io::read_vec();

    // Read `basic_struct`.
    let basic_struct = sp1_zkvm::io::read::<BasicStruct>();

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BASIC_PROGRAM_BYTES_LENGTH);

    // Do some computation on `basic_struct`.
    let basic_struct_output = basic_struct.output();

    // Write reversed `bytes` and `basic_struct_output`
    sp1_zkvm::io::commit_slice(&bytes.into_iter().rev().collect::<Vec<_>>());
    sp1_zkvm::io::commit(&basic_struct_output);
}

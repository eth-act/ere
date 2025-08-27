use risc0_zkvm::guest::env;
use test_utils::guest::{BasicStruct, BASIC_PROGRAM_BYTES_LENGTH};

fn main() {
    // Read `bytes`.
    let bytes = env::read_frame();

    // Read `basic_struct`.
    let basic_struct = env::read::<BasicStruct>();

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BASIC_PROGRAM_BYTES_LENGTH);

    // Do some computation on `basic_struct`.
    let basic_struct_output = basic_struct.output();

    // Write reversed `bytes` and `basic_struct_output`
    env::commit_slice(&bytes.into_iter().rev().collect::<Vec<_>>());
    env::commit(&basic_struct_output);
}

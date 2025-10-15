#![no_main]

use pico_sdk::io::{commit, commit_bytes, read_as, read_vec};
use ere_test_utils::guest::{BasicProgramCore, BasicStruct};

pico_sdk::entrypoint!(main);

pub fn main() {
    // Read `bytes`.
    let bytes = read_vec();

    // Read `basic_struct`.
    let basic_struct = read_as::<BasicStruct>();

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BasicProgramCore::BYTES_LENGTH);

    // Do some computation on `bytes` and `basic_struct`.
    let (rev_bytes, basic_struct_output) = BasicProgramCore::outputs((bytes, basic_struct));

    // Write `rev_bytes` and `basic_struct_output`
    commit_bytes(&rev_bytes);
    commit(&basic_struct_output);
}

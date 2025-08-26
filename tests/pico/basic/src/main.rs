#![no_main]

use pico_sdk::io::{commit, commit_bytes, read_as, read_vec};
use test_utils::guest::BasicStruct;

pico_sdk::entrypoint!(main);

pub fn main() {
    // Read `bytes`.
    let bytes = read_vec();

    // Read `basic_struct`.
    let basic_struct = read_as::<BasicStruct>();
    let basic_struct_output = basic_struct.output();

    // Write reversed `bytes` and `basic_struct_output`
    commit_bytes(&bytes.into_iter().rev().collect::<Vec<_>>());
    commit(&basic_struct_output);
}

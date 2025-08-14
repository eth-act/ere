#![no_main]

use pico_sdk::io::{commit, read_as, read_vec};
use test_utils::guest::BasicStruct;

pico_sdk::entrypoint!(main);

pub fn main() {
    // Read `Hello world` bytes.
    let bytes = read_vec();
    assert_eq!(String::from_utf8_lossy(&bytes), "Hello world");

    // Read `BasicStruct`.
    let basic_struct = read_as::<BasicStruct>();
    let output = basic_struct.output();

    // Write `output`
    commit(&output);
}

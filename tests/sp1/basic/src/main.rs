#![no_main]

use test_utils::guest::BasicStruct;

sp1_zkvm::entrypoint!(main);

pub fn main() {
    // Read `Hello world` bytes.
    let bytes = sp1_zkvm::io::read_vec();
    assert_eq!(String::from_utf8_lossy(&bytes), "Hello world");

    // Read `BasicStruct`.
    let basic_struct = sp1_zkvm::io::read::<BasicStruct>();
    let output = basic_struct.output();

    // Write `output`
    sp1_zkvm::io::commit(&output);
}

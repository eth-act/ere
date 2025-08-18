use openvm::io::{read, read_vec, reveal_u32};
use test_utils::guest::BasicStruct;

fn main() {
    // Read `Hello world` bytes.
    let bytes = read_vec();
    assert_eq!(String::from_utf8_lossy(&bytes), "Hello world");

    // Read `BasicStruct`.
    let basic_struct = read::<BasicStruct>();
    let output = basic_struct.output();

    output.chunks(4).enumerate().for_each(|(idx, bytes)| {
        reveal_u32(u32::from_le_bytes(bytes.try_into().unwrap()), idx);
    });
}

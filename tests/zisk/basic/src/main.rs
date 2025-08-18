#![no_main]

use test_utils::guest::BasicStruct;

ziskos::entrypoint!(main);

fn main() {
    let input = ziskos::read_input();
    let mut input = input.as_slice();

    // Read `Hello world` bytes.
    let bytes: Vec<u8> = bincode::deserialize_from(&mut input).unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), "Hello world");

    // Read `BasicStruct`.
    let basic_struct: BasicStruct = bincode::deserialize_from(&mut input).unwrap();
    let output = basic_struct.output();

    output.chunks(4).enumerate().for_each(|(idx, bytes)| {
        ziskos::set_output(idx, u32::from_le_bytes(bytes.try_into().unwrap()));
    });
}

#![no_main]

use core::array::from_fn;
use test_utils::guest::{BasicStruct, BASIC_PROGRAM_BYTES_LENGTH};

ziskos::entrypoint!(main);

fn main() {
    let input = ziskos::read_input();
    let mut input = input.as_slice();

    // Read `bytes`.
    let bytes: Vec<u8> = bincode::deserialize_from(&mut input).unwrap();

    // Read `basic_struct`.
    let basic_struct: BasicStruct = bincode::deserialize_from(&mut input).unwrap();

    // Check input is fully read.
    assert!(input.is_empty());

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BASIC_PROGRAM_BYTES_LENGTH);

    // Do some computation on `basic_struct`.
    let basic_struct_output = basic_struct.output();

    // Write reversed `bytes` and `basic_struct_output`
    let public_values = core::iter::empty()
        .chain(bytes.into_iter().rev())
        .chain(bincode::serialize(&basic_struct_output).unwrap())
        .collect::<Vec<_>>();
    public_values
        .chunks(4)
        .enumerate()
        .for_each(|(idx, bytes)| {
            let bytes = from_fn(|i| bytes.get(i).copied().unwrap_or_default());
            ziskos::set_output(idx, u32::from_le_bytes(bytes));
        });
}

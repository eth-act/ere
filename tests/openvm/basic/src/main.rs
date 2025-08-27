use core::array::from_fn;
use openvm::io::{read, read_vec, reveal_u32};
use test_utils::guest::{BasicStruct, BASIC_PROGRAM_BYTES_LENGTH};

fn main() {
    // Read `bytes`.
    let bytes = read_vec();

    // Read `basic_struct`.
    let basic_struct = read::<BasicStruct>();

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
            reveal_u32(u32::from_le_bytes(bytes), idx);
        });
}

#![no_main]

use test_utils::guest::{BasicProgramCore, BasicStruct};

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
    assert_eq!(bytes.len(), BasicProgramCore::BYTES_LENGTH);

    // Do some computation on `bytes` and `basic_struct`.
    let outputs = BasicProgramCore::outputs((bytes, basic_struct));

    // Hash `outputs` into digest.
    let digest = BasicProgramCore::sha256_outputs(outputs);

    // Write `digest`
    digest.chunks_exact(4).enumerate().for_each(|(idx, bytes)| {
        ziskos::set_output(idx, u32::from_le_bytes(bytes.try_into().unwrap()))
    });
}

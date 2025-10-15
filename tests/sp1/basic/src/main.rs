#![no_main]

use ere_test_utils::guest::{BasicProgramCore, BasicStruct};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    // Read `bytes`.
    let bytes = sp1_zkvm::io::read_vec();

    // Read `basic_struct`.
    let basic_struct = sp1_zkvm::io::read::<BasicStruct>();

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BasicProgramCore::BYTES_LENGTH);

    // Do some computation on `bytes` and `basic_struct`.
    let (rev_bytes, basic_struct_output) = BasicProgramCore::outputs((bytes, basic_struct));

    // Write `rev_bytes` and `basic_struct_output`
    sp1_zkvm::io::commit_slice(&rev_bytes);
    sp1_zkvm::io::commit(&basic_struct_output);
}

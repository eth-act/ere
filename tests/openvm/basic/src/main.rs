use openvm::io::{read, read_vec, reveal_bytes32};
use test_utils::guest::{BasicProgramCore, BasicStruct};

fn main() {
    // Read `bytes`.
    let bytes = read_vec();

    // Read `basic_struct`.
    let basic_struct = read::<BasicStruct>();

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BasicProgramCore::BYTES_LENGTH);

    // Do some computation on `bytes` and `basic_struct`.
    let outputs = BasicProgramCore::outputs((bytes, basic_struct));

    // Hash `outputs` into digest.
    let digest = BasicProgramCore::sha256_outputs(outputs);

    // Write `digest`
    reveal_bytes32(digest);
}

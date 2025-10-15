use risc0_zkvm::guest::env;
use ere_test_utils::guest::{BasicProgramCore, BasicStruct};

fn main() {
    // Read `bytes`.
    let bytes = env::read_frame();

    // Read `basic_struct`.
    let basic_struct = env::read::<BasicStruct>();

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BasicProgramCore::BYTES_LENGTH);

    // Do some computation on `bytes` and `basic_struct`.
    let (rev_bytes, basic_struct_output) = BasicProgramCore::outputs((bytes, basic_struct));

    // Write `rev_bytes` and `basic_struct_output`
    env::commit_slice(&rev_bytes);
    env::commit(&basic_struct_output);
}

use risc0_zkvm::guest::env;
use test_utils::guest::BasicStruct;

fn main() {
    // Read `Hello world` bytes.
    let bytes = env::read_frame();
    assert_eq!(String::from_utf8_lossy(&bytes), "Hello world");

    // Read `BasicStruct`.
    let basic_struct = env::read::<BasicStruct>();
    let output = basic_struct.output();

    // Write `output`
    env::commit(&output);
}

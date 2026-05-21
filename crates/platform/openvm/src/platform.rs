use core::{array::from_fn, ops::Deref};

use ere_platform_core::Platform;

/// OpenVM [`Platform`] implementation.
///
/// Note that the maximum output size is 32 bytes, and output less than 32
/// bytes will be padded to 32 bytes.
pub struct OpenVMPlatform;

impl Platform for OpenVMPlatform {
    fn read_input() -> impl Deref<Target = [u8]> {
        openvm::io::read_vec()
    }

    fn write_output(output: &[u8]) {
        assert!(
            output.len() <= 32,
            "Maximum output size is 32 bytes, got {} bytes",
            output.len()
        );
        openvm::io::reveal_bytes32(from_fn(|i| output.get(i).copied().unwrap_or(0)));
    }

    fn print(message: &str) {
        openvm::io::print(message)
    }
}

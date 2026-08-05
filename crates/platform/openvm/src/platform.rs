use core::ops::Deref;

use ere_platform_core::Platform;

/// Maximum bytes of output the guest may reveal.
pub const MAX_OUTPUT_BYTES: usize = 256;

/// OpenVM [`Platform`] implementation.
///
/// Note that the maximum output size is 256 bytes, and output less than 256
/// bytes will be padded to 256 bytes.
pub struct OpenVMPlatform;

impl Platform for OpenVMPlatform {
    fn read_input() -> impl Deref<Target = [u8]> {
        openvm::io::read_vec()
    }

    fn write_output(output: &[u8]) {
        assert!(
            output.len() <= MAX_OUTPUT_BYTES,
            "Maximum output size is {MAX_OUTPUT_BYTES} bytes, got {} bytes",
            output.len()
        );
        for (index, chunk) in output.chunks(8).enumerate() {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            openvm::io::reveal_u64(u64::from_le_bytes(word), index);
        }
    }

    fn print(message: &str) {
        openvm::io::print(message)
    }
}

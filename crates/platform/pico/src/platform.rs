use core::ops::Deref;

use ere_platform_core::Platform;

/// Pico [`Platform`] implementation.
///
/// The output is committed to the public values stream, whose SHA-256 digest the proof carries.
pub struct PicoPlatform;

impl Platform for PicoPlatform {
    fn read_input() -> impl Deref<Target = [u8]> {
        pico_sdk::io::read_vec()
    }

    fn write_output(output: &[u8]) {
        pico_sdk::io::commit_bytes(output);
    }

    fn print(message: &str) {
        pico_sdk::riscv_ecalls::syscall_write(1, message.as_ptr(), message.len());
    }
}

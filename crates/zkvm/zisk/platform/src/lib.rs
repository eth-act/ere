#![no_std]

extern crate alloc;

use core::{array::from_fn, ops::Deref};
use ere_platform_trait::LengthPrefixedStdin;
use ziskos::ziskos_definitions::ziskos_config::UART_ADDR;

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use ziskos;

/// ZisK [`Platform`] implementation.
///
/// Note that the maximum output size is 256 bytes, and output size will be
/// padded to multiple of 4.
pub struct ZiskPlatform;

impl Platform for ZiskPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(ziskos::read_input())
    }

    fn write_whole_output(output: &[u8]) {
        assert!(
            output.len() <= 256,
            "Maximum output size is 256 bytes, got {}",
            output.len()
        );
        output.chunks(4).enumerate().for_each(|(idx, chunk)| {
            let value = u32::from_le_bytes(from_fn(|i| chunk.get(i).copied().unwrap_or_default()));
            ziskos::set_output(idx, value)
        });
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        for byte in bytes {
            unsafe {
                core::ptr::write_volatile(UART_ADDR as *mut u8, *byte);
            }
        }
    }
}

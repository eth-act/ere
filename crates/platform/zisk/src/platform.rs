use core::ops::Deref;

use ere_platform_core::{LengthPrefixedStdin, Platform};
use ziskos::ziskos_definitions::ziskos_config::UART_ADDR;

use crate::profile::{SCOPE_REGISTRY, dispatch_profile};

/// ZisK [`Platform`] implementation.
///
/// Note that the maximum output size is 256 bytes, and output size will be
/// padded to multiple of 4.
pub struct ZiskPlatform;

impl Platform for ZiskPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(ziskos::io::read_input_slice())
    }

    fn write_whole_output(output: &[u8]) {
        assert!(
            output.len() <= 256,
            "Maximum output size is 256 bytes, got {}",
            output.len()
        );
        ziskos::io::write(output);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        for byte in bytes {
            unsafe {
                core::ptr::write_volatile(UART_ADDR as *mut u8, *byte);
            }
        }
    }

    fn cycle_scope_start(name: &str) {
        let tag = SCOPE_REGISTRY.get_or_assign_tag(name);
        dispatch_profile!(start, tag);
    }

    fn cycle_scope_end(name: &str) {
        let tag = SCOPE_REGISTRY.get_or_assign_tag(name);
        dispatch_profile!(end, tag);
    }
}

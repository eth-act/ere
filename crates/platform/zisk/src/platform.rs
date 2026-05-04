use core::ops::Deref;

use ere_platform_core::{LengthPrefixedStdin, Platform};
use ziskos::ziskos_definitions::ziskos_config::UART_ADDR;

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
        ziskos::io::commit_slice(output);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        for byte in bytes {
            unsafe {
                core::ptr::write_volatile(UART_ADDR as *mut u8, *byte);
            }
        }
    }

    fn cycle_scope_start(_name: &str) {
        // FIXME: Uncomment when ZisK support profile opcode in program setup
        // #[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
        // ziskos::ziskos_syscall!(
        //     ziskos::SYSCALL_PROFILE_ID,
        //     ziskos::PROFILE_REPORT_START_COST_ID,
        //     &_name as *const &str as usize
        // );
    }

    fn cycle_scope_end(_name: &str) {
        // FIXME: Uncomment when ZisK support profile opcode in program setup
        // #[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
        // ziskos::ziskos_syscall!(
        //     ziskos::SYSCALL_PROFILE_ID,
        //     ziskos::PROFILE_REPORT_END_COST_ID,
        //     &_name as *const &str as usize
        // )
    }
}

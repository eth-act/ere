use alloc::format;

use ere_platform_core::Platform;

/// SP1 [`Platform`] implementation.
pub struct SP1Platform;

/// `read_input`, `write_output` and `abort` are the trait's defaults, on the zkvm-standards C
/// functions that `libzkevm` exports.
impl Platform for SP1Platform {
    fn print(message: &str) {
        sp1_zkvm::io::write(1, message.as_bytes());
    }

    fn cycle_scope_start(name: &str) {
        Self::print(&format!("cycle-tracker-report-start: {name}"))
    }

    fn cycle_scope_end(name: &str) {
        Self::print(&format!("cycle-tracker-report-end: {name}"))
    }
}

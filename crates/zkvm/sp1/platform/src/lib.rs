#![no_std]

extern crate alloc;

use alloc::format;
use core::ops::Deref;
use ere_platform_trait::LengthPrefixedStdin;

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use sp1_zkvm;

/// SP1 [`Platform`] implementation.
pub struct SP1Platform;

impl Platform for SP1Platform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(sp1_zkvm::io::read_vec())
    }

    fn write_whole_output(output: &[u8]) {
        sp1_zkvm::io::commit_slice(output);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        sp1_zkvm::syscalls::sys_write(1, bytes.as_ptr(), bytes.len());
    }

    fn cycle_scope_start(name: &str) {
        Self::print(&format!("cycle-tracker-report-start: {name}"))
    }

    fn cycle_scope_end(name: &str) {
        Self::print(&format!("cycle-tracker-report-end: {name}"))
    }
}

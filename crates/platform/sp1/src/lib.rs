#![no_std]

extern crate alloc;

use alloc::format;
use core::ops::Deref;
use ere_platform_core::LengthPrefixedStdin;

pub use ere_platform_core::{Digest, OutputHashedPlatform, Platform};
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
        sp1_zkvm::io::write(1, message.as_bytes());
    }

    fn cycle_scope_start(name: &str) {
        Self::print(&format!("cycle-tracker-report-start: {name}"))
    }

    fn cycle_scope_end(name: &str) {
        Self::print(&format!("cycle-tracker-report-end: {name}"))
    }
}

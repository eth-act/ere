#![no_std]

extern crate alloc;

use alloc::format;
use core::ops::Deref;
use ere_platform_trait::LengthPrefixedStdin;

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use pico_sdk;

/// Pico [`Platform`] implementation.
pub struct PicoPlatform;

impl Platform for PicoPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(pico_sdk::io::read_vec())
    }

    fn write_whole_output(output: &[u8]) {
        pico_sdk::io::commit_bytes(output);
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        pico_sdk::riscv_ecalls::sys_write(1, bytes.as_ptr(), bytes.len());
    }

    fn cycle_scope_start(name: &str) {
        Self::print(&format!("cycle-tracker-start: {name}"))
    }

    fn cycle_scope_end(name: &str) {
        Self::print(&format!("cycle-tracker-end: {name}"))
    }
}

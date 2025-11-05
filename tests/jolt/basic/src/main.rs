#![no_std]
#![no_main]

extern crate alloc;

use ere_platform_jolt::{jolt, JoltPlatform};
use ere_test_utils::program::{basic::BasicProgram, Program};

#[jolt::provable(guest_only)]
fn main() {
    BasicProgram::run::<JoltPlatform>();
}

#![no_std]
#![no_main]

extern crate alloc;

use ere_platform_jolt::{jolt, JoltPlatform};
use ere_test_utils::{
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

#[jolt::provable(guest_only)]
fn main() {
    BasicProgram::<BincodeLegacy>::run::<JoltPlatform>();
}

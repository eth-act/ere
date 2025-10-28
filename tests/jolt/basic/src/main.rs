#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::mem;
use ere_test_utils::{
    guest::Platform,
    program::{basic::BasicProgram, Program},
};

static mut INPUT: Vec<u8> = Vec::new();
static mut OUTPUT: Vec<u8> = Vec::new();

struct JoltPlatform;

impl Platform for JoltPlatform {
    fn read_input() -> Vec<u8> {
        unsafe { mem::take(&mut INPUT) }
    }

    fn write_output(output: &[u8]) {
        unsafe { mem::replace(&mut OUTPUT, output.to_vec()) };
    }
}

#[jolt::provable(guest_only)]
fn main(input: Vec<u8>) -> Vec<u8> {
    unsafe { mem::replace(&mut INPUT, input) };
    BasicProgram::run::<JoltPlatform>();
    unsafe { mem::take(&mut OUTPUT) }
}

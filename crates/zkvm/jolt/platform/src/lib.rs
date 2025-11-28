#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::{marker::PhantomData, slice};
use ere_platform_trait::output_hasher::OutputHasher;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use jolt_sdk as jolt;

// FIXME: Because the crate `jolt-common` is not `no_std` compatible, so we have
//        to temporarily copy-paste these contant and memory layout calculation.
pub const RAM_START_ADDRESS: u64 = 0x80000000;

pub const DEFAULT_MEMORY_SIZE: u64 = 32 * 1024 * 1024;
pub const DEFAULT_STACK_SIZE: u64 = 4096;
pub const DEFAULT_MAX_INPUT_SIZE: u64 = 4096;
pub const DEFAULT_MAX_OUTPUT_SIZE: u64 = 4096;
pub const DEFAULT_MAX_TRACE_LENGTH: u64 = 1 << 24;

pub struct JoltMemoryLayout {
    max_input_size: u64,
    max_output_size: u64,
    input_start: u64,
    output_start: u64,
}

pub trait JoltMemoryConfig {
    const MAX_INPUT_SIZE: u64;
    const MAX_OUTPUT_SIZE: u64;
    const STACK_SIZE: u64;
    const MEMORY_SIZE: u64;

    // According to https://github.com/a16z/jolt/blob/v0.3.0-alpha/common/src/jolt_device.rs#L181.
    fn memory_layout() -> JoltMemoryLayout {
        let max_input_size = Self::MAX_INPUT_SIZE.next_multiple_of(8);
        let max_output_size = Self::MAX_OUTPUT_SIZE.next_multiple_of(8);

        let io_region_bytes = max_input_size
            .checked_add(max_output_size)
            .unwrap()
            .checked_add(16)
            .unwrap();
        let io_region_words = (io_region_bytes / 8).next_power_of_two();
        let io_bytes = io_region_words.checked_mul(8).unwrap();

        let input_start = RAM_START_ADDRESS.checked_sub(io_bytes).unwrap();
        let output_start = input_start.checked_add(max_input_size).unwrap();

        JoltMemoryLayout {
            max_input_size,
            max_output_size,
            input_start,
            output_start,
        }
    }
}

pub struct DefaulJoltMemoryConfig;

impl JoltMemoryConfig for DefaulJoltMemoryConfig {
    const MAX_INPUT_SIZE: u64 = DEFAULT_MAX_INPUT_SIZE;
    const MAX_OUTPUT_SIZE: u64 = DEFAULT_MAX_OUTPUT_SIZE;
    const STACK_SIZE: u64 = DEFAULT_STACK_SIZE;
    const MEMORY_SIZE: u64 = DEFAULT_MEMORY_SIZE;
}

/// Jolt [`Platform`] implementation.
pub struct JoltPlatform<C = DefaulJoltMemoryConfig, H = IdentityOutput>(PhantomData<(C, H)>);

impl<C: JoltMemoryConfig, H: OutputHasher> Platform for JoltPlatform<C, H> {
    fn read_whole_input() -> Vec<u8> {
        let memory_layout = C::memory_layout();
        let input_ptr = memory_layout.input_start as *const u8;
        let max_input_len = memory_layout.max_input_size as usize;
        let input_slice = unsafe { slice::from_raw_parts(input_ptr, max_input_len) };
        let (input, _) = jolt::postcard::take_from_bytes(input_slice).unwrap();
        input
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        let memory_layout = C::memory_layout();
        let output_ptr = memory_layout.output_start as *mut u8;
        let max_output_len = memory_layout.max_output_size as usize;
        let output_slice = unsafe { core::slice::from_raw_parts_mut(output_ptr, max_output_len) };
        jolt::postcard::to_slice(&*hash, output_slice).unwrap_or_else(|err| match err {
            jolt::postcard::Error::SerializeBufferFull => {
                panic!("Maximum output size is {max_output_len} bytes")
            }
            err => panic!("`postcard::to_slice` failed: {err:?}"),
        });
    }

    fn print(message: &str) {
        jolt::print(message);
    }
}

#![no_std]

extern crate alloc;

use core::{marker::PhantomData, ops::Deref};
use jolt_common::constants::{
    DEFAULT_MAX_INPUT_SIZE, DEFAULT_MAX_OUTPUT_SIZE, DEFAULT_MAX_TRUSTED_ADVICE_SIZE,
    DEFAULT_MAX_UNTRUSTED_ADVICE_SIZE, DEFAULT_MEMORY_SIZE, DEFAULT_STACK_SIZE,
};
use jolt_common::jolt_device::{MemoryConfig, MemoryLayout};

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use jolt_sdk as jolt;

// According to https://github.com/a16z/jolt/blob/6dcd401/common/src/jolt_device.rs
pub trait JoltMemoryConfig {
    const MAX_INPUT_SIZE: u64;
    const MAX_TRUSTED_ADVICE_SIZE: u64;
    const MAX_UNTRUSTED_ADVICE_SIZE: u64;
    const MAX_OUTPUT_SIZE: u64;
    const STACK_SIZE: u64;
    const MEMORY_SIZE: u64;

    fn memory_layout() -> MemoryLayout {
        MemoryLayout::new(&MemoryConfig {
            max_input_size: Self::MAX_INPUT_SIZE,
            max_trusted_advice_size: Self::MAX_TRUSTED_ADVICE_SIZE,
            max_untrusted_advice_size: Self::MAX_UNTRUSTED_ADVICE_SIZE,
            max_output_size: Self::MAX_OUTPUT_SIZE,
            stack_size: Self::STACK_SIZE,
            memory_size: Self::MEMORY_SIZE,
            program_size: Some(0),
        })
    }
}

pub struct DefaultJoltMemoryConfig;

impl JoltMemoryConfig for DefaultJoltMemoryConfig {
    const MAX_INPUT_SIZE: u64 = DEFAULT_MAX_INPUT_SIZE;
    const MAX_TRUSTED_ADVICE_SIZE: u64 = DEFAULT_MAX_TRUSTED_ADVICE_SIZE;
    const MAX_UNTRUSTED_ADVICE_SIZE: u64 = DEFAULT_MAX_UNTRUSTED_ADVICE_SIZE;
    const MAX_OUTPUT_SIZE: u64 = DEFAULT_MAX_OUTPUT_SIZE;
    const STACK_SIZE: u64 = DEFAULT_STACK_SIZE;
    const MEMORY_SIZE: u64 = DEFAULT_MEMORY_SIZE;
}

/// Jolt [`Platform`] implementation.
pub struct JoltPlatform<C = DefaultJoltMemoryConfig>(PhantomData<C>);

impl<C: JoltMemoryConfig> Platform for JoltPlatform<C> {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        let memory_layout = C::memory_layout();
        let untrusted_advice_ptr = memory_layout.untrusted_advice_start as *const u8;
        let max_untrusted_advice_len = memory_layout.max_untrusted_advice_size as usize;
        assert!(max_untrusted_advice_len > 4);
        let len_bytes = unsafe { core::slice::from_raw_parts(untrusted_advice_ptr, 4) };
        let len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
        assert!(
            len <= max_untrusted_advice_len - 4,
            "Maximum input size is {} bytes, got {len}",
            max_untrusted_advice_len - 4,
        );
        unsafe { core::slice::from_raw_parts(untrusted_advice_ptr.add(4), len) }.to_vec()
    }

    fn write_whole_output(output: &[u8]) {
        let memory_layout = C::memory_layout();
        let output_ptr = memory_layout.output_start as *mut u8;
        let max_output_len = memory_layout.max_output_size as usize;
        let len = output.len();
        assert!(
            len <= max_output_len - 4,
            "Maximum output size is {} bytes, got {len}",
            max_output_len - 4,
        );
        let output_slice = unsafe { core::slice::from_raw_parts_mut(output_ptr, len + 4) };
        output_slice[..4].copy_from_slice(&(output.len() as u32).to_le_bytes());
        output_slice[4..].copy_from_slice(output);
    }

    fn print(message: &str) {
        jolt::print(message);
    }
}

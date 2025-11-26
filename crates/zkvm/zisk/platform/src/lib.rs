#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::{array::from_fn, marker::PhantomData};
use ere_platform_trait::output_hasher::OutputHasher;
use ziskos::ziskos_definitions::ziskos_config::UART_ADDR;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum},
};
pub use ziskos;

/// ZisK [`Platform`] implementation.
///
/// Note that the maximum output size is 256 bytes, and output size will be
/// padded to multiple of 4.
pub struct ZiskPlatform<H = IdentityOutput>(PhantomData<H>);

impl<H: OutputHasher> Platform for ZiskPlatform<H> {
    fn read_whole_input() -> Vec<u8> {
        ziskos::read_input()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        assert!(hash.len() <= 256, "Maximum output size is 256 bytes");
        hash.chunks(4).enumerate().for_each(|(idx, chunk)| {
            let value = u32::from_le_bytes(from_fn(|i| chunk.get(i).copied().unwrap_or_default()));
            ziskos::set_output(idx, value)
        });
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        for byte in bytes {
            unsafe {
                core::ptr::write_volatile(UART_ADDR as *mut u8, *byte);
            }
        }
    }
}

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;
use ere_platform_trait::output_hasher::FixedOutputHasher;
use ziskos::ziskos_definitions::ziskos_config::UART_ADDR;

pub use ere_platform_trait::{
    Platform,
    output_hasher::{IdentityOutput, PaddedOutput, digest::typenum::U32},
};
pub use ziskos;

pub struct ZiskPlatform<H>(PhantomData<H>);

impl<H: FixedOutputHasher<OutputSize = U32>> Platform for ZiskPlatform<H> {
    fn read_whole_input() -> Vec<u8> {
        ziskos::read_input()
    }

    fn write_whole_output(output: &[u8]) {
        let hash = H::output_hash(output);
        hash.chunks_exact(4).enumerate().for_each(|(idx, bytes)| {
            ziskos::set_output(idx, u32::from_le_bytes(bytes.try_into().unwrap()))
        });
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        for i in 0..bytes.len() {
            unsafe {
                core::ptr::write_volatile(UART_ADDR as *mut u8, bytes[i]);
            }
        }
    }
}

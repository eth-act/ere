#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::{array, iter::repeat_with, ops::Deref};

pub use airbender_riscv_common as riscv_common;
pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};

/// Airbender [`Platform`] implementation.
///
/// Note that the maximum output size is 32 bytes, and output less than 32
/// bytes will be padded to 32 bytes.
pub struct AirbenderPlatform;

impl Platform for AirbenderPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        let len = riscv_common::csr_read_word() as usize;
        repeat_with(riscv_common::csr_read_word)
            .take(len.div_ceil(4))
            .flat_map(u32::to_le_bytes)
            .take(len)
            .collect::<Vec<_>>()
    }

    fn write_whole_output(output: &[u8]) {
        assert!(
            output.len() <= 32,
            "Maximum output size is 32 bytes, got {} bytes",
            output.len()
        );
        let words = array::from_fn(|i| u32::from_le_bytes(array::from_fn(|j| output[4 * i + j])));
        riscv_common::zksync_os_finish_success(&words);
    }

    fn print(_message: &str) {
        #[cfg(feature = "uart")]
        core::fmt::Write::write_str(&mut riscv_common::QuasiUART::new(), _message).unwrap();
    }
}

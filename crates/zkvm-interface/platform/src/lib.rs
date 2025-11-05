#![no_std]

extern crate alloc;

use alloc::vec::Vec;

#[cfg(feature = "output-hasher")]
pub mod output_hasher;

/// Platform dependent methods.
pub trait Platform {
    /// Read the whole input at once from host.
    ///
    /// Note that this function should only be called once.
    fn read_whole_input() -> Vec<u8>;

    /// Write the whole output at once to host.
    ///
    /// Note that this function should only be called once.
    fn write_whole_output(output: &[u8]);
}

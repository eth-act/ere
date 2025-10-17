use alloc::vec::Vec;

pub use sha2::{Digest, Sha256};

/// Platform dependent methods.
pub trait Platform {
    /// Read the whole input at once from host.
    fn read_input() -> Vec<u8>;

    /// Write the whole output at once to host.
    fn write_output(output: &[u8]);
}

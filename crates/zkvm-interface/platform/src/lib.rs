#![no_std]

use alloc::vec::Vec;
use core::ops::Deref;

extern crate alloc;

pub mod output_hasher;

/// Platform dependent methods.
pub trait Platform {
    /// Reads the whole input at once from host.
    ///
    /// The `stdin` passed must have a LE u32 length prefix for efficiency
    /// reason. Use `Input::new().with_prefixed_stdin(stdin)` for convenience.
    ///
    /// Note that this function should only be called once.
    fn read_whole_input() -> impl Deref<Target = [u8]>;

    /// Writes the whole output at once to host.
    ///
    /// Note that this function should only be called once.
    fn write_whole_output(output: &[u8]);

    /// Prints a message to the host environment.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    fn print(message: &str);

    /// Returns the current cycle count.
    ///
    /// Note that this function will return `0` if the platform doesn't support.
    #[inline]
    fn cycle_count() -> u64 {
        0
    }

    /// Enters a cycle scope of `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    #[inline]
    fn cycle_scope_start(_name: &str) {}

    /// Exits a cycle scope of `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    #[inline]
    fn cycle_scope_end(_name: &str) {}

    /// Runs a given function `f` within a cycle scope `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    #[inline]
    fn cycle_scope<T>(name: &str, f: impl FnOnce() -> T) -> T {
        Self::cycle_scope_start(name);
        let t = f();
        Self::cycle_scope_end(name);
        t
    }
}

/// Stdin with a LE u32 length prefix.
///
/// Dereferencing it returns slice to the actual data.
pub struct LengthPrefixedStdin(Vec<u8>);

impl Deref for LengthPrefixedStdin {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0[4..]
    }
}

impl LengthPrefixedStdin {
    pub fn new(stdin: Vec<u8>) -> Self {
        let len = u32::from_le_bytes(stdin[..4].try_into().unwrap());
        assert_eq!(
            stdin.len(),
            len as usize + 4,
            "stdin must have a LE u32 length prefix"
        );
        Self(stdin)
    }
}

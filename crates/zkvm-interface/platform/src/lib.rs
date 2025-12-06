#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::{marker::PhantomData, ops::Deref};

pub use digest::Digest;

/// Platform dependent methods.
pub trait Platform {
    /// Reads the whole input at once from host.
    ///
    /// The stdin passed must have a LE u32 length prefix, because some zkVMs
    /// don't provide access to the stdin length.
    /// Use `Input::new().with_prefixed_stdin(stdin)` for convenience.
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

/// Wrapper for `Platform` implementation that hashes output before calling
/// the inner `P::write_whole_output`.
pub struct OutputHashedPlatform<P, D>(PhantomData<(P, D)>);

impl<P, D> Platform for OutputHashedPlatform<P, D>
where
    P: Platform,
    D: Digest,
{
    #[inline]
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        P::read_whole_input()
    }

    #[inline]
    fn write_whole_output(output: &[u8]) {
        P::write_whole_output(&D::digest(output));
    }

    #[inline]
    fn print(message: &str) {
        P::print(message);
    }

    #[inline]
    fn cycle_count() -> u64 {
        P::cycle_count()
    }

    #[inline]
    fn cycle_scope_start(name: &str) {
        P::cycle_scope_start(name)
    }

    #[inline]
    fn cycle_scope_end(name: &str) {
        P::cycle_scope_end(name)
    }

    #[inline]
    fn cycle_scope<T>(name: &str, f: impl FnOnce() -> T) -> T {
        P::cycle_scope(name, f)
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
        assert!(
            stdin.len() >= 4,
            "stdin must have a LE u32 length prefix; use Input::with_prefixed_length(stdin) on the host side"
        );
        let len = u32::from_le_bytes(stdin[..4].try_into().unwrap()) as usize;
        assert_eq!(
            len,
            stdin.len() - 4,
            "Length mismatch: stdin length prefix indicated {len} bytes, but got {} bytes; use Input::with_prefixed_length(stdin) on the host side",
            stdin.len() - 4
        );
        Self(stdin)
    }
}

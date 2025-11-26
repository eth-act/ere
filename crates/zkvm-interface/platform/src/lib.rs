#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub mod output_hasher;

/// Platform dependent methods.
pub trait Platform {
    /// Reads the whole input at once from host.
    ///
    /// Note that this function should only be called once.
    fn read_whole_input() -> Vec<u8>;

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
    /// Note that this function will return `None` if the platform doesn't support.
    fn cycle_count() -> Option<u64> {
        None
    }

    /// Enters a cycle scope of `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    fn cycle_scope_start(_name: &str) {}

    /// Exits a cycle scope of `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    fn cycle_scope_end(_name: &str) {}

    /// Runs a given function `f` within a cycle scope `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    fn cycle_scope(name: &str, f: impl Fn()) {
        Self::cycle_scope_start(name);
        f();
        Self::cycle_scope_end(name);
    }
}

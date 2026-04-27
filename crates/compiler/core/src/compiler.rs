use core::error::Error;
use std::path::Path;

use crate::Elf;

/// Compiler trait for compiling guest programs into an [`Elf`] binary.
pub trait Compiler {
    type Error: 'static + Send + Sync + Error;

    /// Compiles the program and returns the [`Elf`]
    ///
    /// # Arguments
    /// * `guest_directory` - The path to the guest program directory
    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error>;
}

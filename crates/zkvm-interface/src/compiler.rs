use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

/// Compiler trait for compiling programs into an opaque sequence of bytes.
pub trait Compiler {
    type Error: std::error::Error + Send + Sync + 'static;
    type Program: Clone + Send + Sync + Serialize + DeserializeOwned;

    /// Compiles the program and returns the program
    ///
    /// # Arguments
    /// * `guest_directory` - The path to the guest program directory
    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error>;
}

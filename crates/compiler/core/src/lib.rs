#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod compiler;
mod elf;

pub use crate::{compiler::Compiler, elf::Elf};

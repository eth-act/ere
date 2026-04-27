#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

mod platform;

pub use ere_platform_core::Platform;
pub use risc0_zkvm;

pub use crate::platform::Risc0Platform;

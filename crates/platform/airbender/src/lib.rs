#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

mod platform;

pub use airbender_riscv_common as riscv_common;
pub use ere_platform_core::Platform;

pub use crate::platform::AirbenderPlatform;

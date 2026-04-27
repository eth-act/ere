#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

mod platform;

pub use ere_platform_core::Platform;
pub use openvm;

pub use crate::platform::OpenVMPlatform;

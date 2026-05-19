#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod platform;

pub use ere_platform_core::Platform;
pub use zisk_zkvm_interface as zkvm_interface;
pub use ziskos;

pub use crate::platform::ZiskPlatform;

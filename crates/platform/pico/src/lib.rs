#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod platform;

pub use ere_platform_core::Platform;
pub use pico_sdk;

pub use crate::platform::PicoPlatform;

#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod platform;

pub use ere_platform_core::Platform;
/// `pico-sdk` also exports the `zkvm_accelerators.h` functions with C ABI inside the zkVM, so
/// a guest built against `zkvm-interface` links them by depending on this crate.
pub use pico_sdk;

pub use crate::platform::PicoPlatform;

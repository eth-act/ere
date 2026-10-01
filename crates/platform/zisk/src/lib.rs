// Only for ZisK guests (`riscv64ima-zisk-zkvm-elf`): built for any other target, the crate is
// empty.
#![cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod platform;

pub use ere_platform_core::Platform;
pub use ziskos;

pub use crate::platform::ZiskPlatform;

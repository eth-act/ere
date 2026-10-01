// Only for SP1 guests (`riscv64im-succinct-zkvm-elf`): built for any other target, the crate is
// empty.
#![cfg(all(target_os = "zkvm", target_vendor = "succinct"))]
#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

use sp1_libzkevm as _;

mod platform;

pub use ere_platform_core::Platform;
pub use sp1_zkvm;

pub use crate::platform::SP1Platform;

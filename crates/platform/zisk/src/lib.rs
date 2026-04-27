#![no_std]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

extern crate alloc;

mod platform;
mod profile;

pub use ere_platform_core::Platform;
pub use ziskos;

pub use crate::{platform::ZiskPlatform, profile::check_cycle_scope_names};

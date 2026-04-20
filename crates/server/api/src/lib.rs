#![cfg_attr(not(test), warn(unused_crate_dependencies))]

#[rustfmt::skip]
mod api;

#[cfg(test)]
mod test;

pub use api::*;

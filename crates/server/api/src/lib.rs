#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Generated code: `async_trait` marks the boxed futures it returns `#[must_use]`, which clippy
// (since Rust 1.99) reports as `double_must_use`.
#[allow(clippy::double_must_use)]
#[rustfmt::skip]
mod api;

#[cfg(test)]
mod test;

pub use api::*;

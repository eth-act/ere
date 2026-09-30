//! Platform for guests that name no zkVM.
//!
//! A guest built on this crate is linked against a zkVM SDK (`libzkvm.a` and `zkvm.ld`), which
//! provides the [zkvm-standards] guest ABI: `read_input`, `write_output`, the `zkvm_*`
//! accelerators and `abort`, and whose linker script bounds the guest's heap with `_heap_start` and
//! `_heap_end`. This crate is the guest's side of that ABI:
//!
//! - [`ZkvmPlatform`], the [`Platform`] on the standard input and output;
//! - [`accelerators`], a safe API for the standard accelerators;
//! - with the `runtime` feature (default), on bare-metal targets, the guest's global allocator (a
//!   bump allocator from `_heap_start` to `_heap_end`) and panic handler.
//!
//! [zkvm-standards]: https://github.com/eth-act/zkvm-standards

#![no_std]

extern crate alloc;

pub mod accelerators;
#[cfg(all(feature = "runtime", target_os = "none"))]
mod runtime;

pub use ere_platform_core::Platform;

/// [`Platform`] on the standard `read_input` and `write_output`.
#[derive(Debug)]
pub struct ZkvmPlatform;

impl Platform for ZkvmPlatform {}

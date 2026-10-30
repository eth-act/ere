//! Pico [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_pico_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-pico` dependency.
//!
//! `pico-vm` needs nightly features, so build with the nightly the installer adds, for example
//! `cargo +nightly-2026-09-27 test -p ere-prover-pico`.
//!
//! To use with GPU proving support, make sure CUDA 12.8 or newer is installed, and turn on the
//! `cuda` feature.
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-pico`](https://github.com/eth-act/ere/tree/master/crates/compiler/pico) crate.
//!
//! # `zkVMProver` implementation
//!
//! ## Supported `ProverResource`
//!
//! | Resource  | Supported |
//! | --------- | :-------: |
//! | `Cpu`     |    Yes    |
//! | `Gpu`     |    Yes    |
//! | `Network` |    No     |
//! | `Cluster` |    No     |
//!
//! `Gpu` needs the `cuda` feature, and a build with it supports only `Gpu`: `pico-vm/cuda`
//! makes every proving machine a GPU one. Use a build without `cuda` for `Cpu`.
//!
//! On the GPU, a failing guest (for example an invalid input) makes `prove` return an error and
//! the process keeps proving. A GPU failure leaves the process unable to prove on the GPU again;
//! restart it, since `setup` rebuilds the prover but not the GPU pipeline.
//!
//! Construction reserves most of the card's memory, and GPU proves pin host memory for event
//! buffers (tens of GiB), both for the life of the process. Every visible card is used; the GPU
//! memory tier follows the smallest card, and `PICO_GPU_MEM=4090` forces the 24 GB tier.
//!
//! `G1_JIT=1` and `Z1_JIT=1` JIT-compile the GPU constraint kernels. It is off by default: the
//! first prove of a program compiles them, which can take tens of minutes, so mount
//! `ERE_PICO_CACHE_VOLUME` to keep the compiled kernels across containers.
//!
//! ## Cost estimation
//!
//! The unit is main-trace cells of the RISC-V chips: rows times width, summed
//! over every chunk, before padding. Recursion is not counted.
//!
//! | Component    | Meaning                                                        |
//! | ------------ | -------------------------------------------------------------- |
//! | `rv64`       | The instruction set: CPU, ALU, memory access and ecall chips   |
//! | `precompile` | The precompile chips and the syscall tables that dispatch them |
//! | `system`     | Byte, program, memory and global lookup tables                 |
//!
//! ## Profile
//!
//! `profile` returns `Error::ProfileUnsupported`.
//!
//! ## Security
//!
//! Proofs are proved in whitelist mode at log stacked height 23, the only
//! operating point the shipped whitelist of recursion keys covers. The Pico 3.0
//! MLE backend targets 100 bits of provable round-by-round soundness.
//!
//! [`install_pico_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_pico_sdk.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
mod prover;

pub use ere_prover_core::*;
pub use ere_verifier_pico::*;

pub use crate::{error::Error, prover::PicoProver};

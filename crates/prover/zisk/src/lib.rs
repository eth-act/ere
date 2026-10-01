//! ZisK [`zkVMProver`] implementation.
//!
//! # Requirements
//!
//! To install all requirements, run [`install_zisk_sdk.sh`] from the Ere
//! repository at the same git revision as your `ere-prover-zisk` dependency.
//!
//! GPU proving requires the `cuda` Cargo feature and CUDA 12.9 installed.
//!
//! Local proving downloads the pinned proving key archive into
//! `$HOME/.zisk/provingKey` before the first setup, unless a previous download
//! left its `.md5` marker there. `ERE_ZISK_SETUP_ON_INIT` moves this into
//! construction, so allow for the download time there.
//!
//! Set `ZISK_USE_INSTALLED=1`, so the setup and the first execution of a program build its ASM
//! services from the SDK that [`install_zisk_sdk.sh`] installs. Otherwise, when
//! `cargo` is on `PATH`, the setup builds them in the cargo checkout of the ZisK
//! crates.
//!
//! ## `zkVMProver` requirements
//!
//! - Installation via [`ziskup`]
//!
//! # `Compiler` implementation
//!
//! See the separate [`ere-compiler-zisk`](https://github.com/eth-act/ere/tree/master/crates/compiler/zisk) crate.
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
//! | `Cluster` |    Yes    |
//!
//! ## Execution
//!
//! On x86_64 Linux, execution runs the guest as native x86-64 code that ZisK generates from the ELF
//! (the ASM emulator in Fast mode). This is much faster than the Rust emulator for long runs.
//!
//! - The first execution of a program compiles it, which takes seconds. Later executions use the
//!   cached binary.
//! - Each execution runs on an idle service of the program, and concurrent executions start more
//!   services, up to `ERE_ZISK_EXECUTE_CONCURRENCY`. Each service uses up to 1.7 GiB of `/dev/shm`.
//!   In Docker, raise the 64 MiB default with `--shm-size`.
//! - A run above the 2^36-step limit of the ZisK prover fails with `EmulatorNotTerminated`. The ASM
//!   emulator does not count steps as it runs, so a service that takes longer than
//!   `ERE_ZISK_EXECUTE_TIMEOUT_SECS` (5 minutes by default) to start or to run is killed, and the
//!   execution fails with `AsmEmulatorTimeout`.
//! - The native code does not check guest memory accesses, as in the ASM services of the prover. A
//!   load past the input reads zeros, as in the Rust emulator, except at the control words from
//!   `0x70000000`, which the services of the prover map there too. A guest can also read and write
//!   the memory of its service process, so execute and prove only trusted programs.
//! - Guest prints do not appear.
//! - Other targets and guests with the `cycle-scope` feature use the Rust emulator.
//!
//! Execution, cost estimation and proving reject a stdin above 768 MiB - 16 bytes, because the ASM
//! emulator maps its control input over the rest of the input region.
//!
//! ## Cost estimation
//!
//! The unit is trace cells. A table costs its rows times its width.
//!
//! | Component    | Meaning                                     |
//! | ------------ | ------------------------------------------- |
//! | `base`       | Fixed cost of the ROM and the lookup tables |
//! | `precompile` | Accelerated operations, such as hashes      |
//! | `memory`     | Memory reads, writes and alignment work     |
//! | `opcode`     | Plain RISC-V instructions                   |
//! | `main`       | The main table, one entry per step          |
//!
//! The emulator runs with statistics turned on. That setting also prints its own
//! report to stdout. ZisK sums the five components into the total, so a mismatch
//! means the estimator misread the report and the estimate fails.
//!
//! ## Environment variables
//!
//! | Variable                               | Type  | Default        | Description                                            |
//! | -------------------------------------- | ----- | -------------- | ------------------------------------------------------ |
//! | `ERE_ZISK_SETUP_ON_INIT`               | Flag  |                | Setup local prover on `new` and `setup`, not lazily    |
//! | `ERE_ZISK_UNLOCK_MAPPED_MEMORY`        | Flag  |                | Configure the prover to unlock mapped memory           |
//! | `ERE_ZISK_MINIMAL_MEMORY`              | Flag  |                | Configure the prover to use minimal memory             |
//! | `ERE_ZISK_MAX_STREAMS`                 | Value |                | Configure the prover max streams                       |
//! | `ERE_ZISK_NUMBER_THREADS_WITNESS`      | Value |                | Configure the prover number of witness threads         |
//! | `ERE_ZISK_MAX_WITNESS_STORED`          | Value |                | Configure the prover max witness stored                |
//! | `ERE_ZISK_CLUSTER_PROVE_TIMEOUT_SECS`  | Value |                | Timeout for the cluster client prove job               |
//! | `ERE_ZISK_EXECUTE_TIMEOUT_SECS`        | Value | `300`          | Timeout for the start and each run of an ASM service   |
//! | `ERE_ZISK_EXECUTE_CONCURRENCY`         | Value | CPUs, max 32   | Services that execute one program at once              |
//!
//! [`install_zisk_sdk.sh`]: https://github.com/eth-act/ere/blob/master/scripts/sdk_installers/install_zisk_sdk.sh
//! [`ziskup`]: https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cost;
mod error;
mod executor;
mod prover;
mod sdk;

pub use ere_prover_core::*;
pub use ere_verifier_zisk::*;

pub use crate::{error::Error, prover::ZiskProver};

<p align="center">
  <img src="assets/logo-blue-white.svg" alt="Ere logo" width="260"/>
</p>

<h1 align="center">Ere – Unified zkVM Interface & Toolkit</h1>

<p align="center">
  <b>Compile. Execute. Prove. Verify.</b><br/>
  One ergonomic Rust API, multiple zero‑knowledge virtual machines.
</p>

---

## Table of Contents

- [Table of Contents](#table-of-contents)
- [Supported Rust Versions (MSRV)](#supported-rust-versions-msrv)
- [Overview](#overview)
- [Architecture](#architecture)
  - [The Interface](#the-interface)
  - [Communication between Host and Guest](#communication-between-host-and-guest)
    - [Reading Private Values from Host](#reading-private-values-from-host)
    - [Writing Public Values to Host](#writing-public-values-to-host)
- [Supported zkVMs](#supported-zkvms)
- [Examples](#examples)
  - [With SDK Installation](#with-sdk-installation)
    - [1. Install SDKs](#1-install-sdks)
    - [2. Create Guest Program](#2-create-guest-program)
    - [3. Create Host](#3-create-host)
  - [Docker-Only Setup](#docker-only-setup)
    - [1. Create Guest Program](#1-create-guest-program)
    - [2. Create Host](#2-create-host)
- [Directory Layout](#directory-layout)
- [Contributing](#contributing)
- [Disclaimer](#disclaimer)
- [License](#license)

## Supported Rust Versions (MSRV)

The current MSRV (minimum supported rust version) is 1.88.

## Overview

This repository contains the following crates:

- Traits
  - [`ere-zkvm-interface`] - `Compiler` and `zkVM` traits for zkVM host operations
  - [`ere-platform-trait`] - `Platform` trait for guest program
- Per-zkVM implementations for [`ere-zkvm-interface`] (host)
  - [`ere-airbender`]
  - [`ere-jolt`]
  - [`ere-miden`]
  - [`ere-nexus`]
  - [`ere-openvm`]
  - [`ere-pico`]
  - [`ere-risc0`]
  - [`ere-sp1`]
  - [`ere-ziren`]
  - [`ere-zisk`]
- Per-zkVM implementations for [`ere-platform-trait`] (guest)
  - [`ere-platform-airbender`]
  - [`ere-platform-jolt`]
  - [`ere-platform-nexus`]
  - [`ere-platform-openvm`]
  - [`ere-platform-pico`]
  - [`ere-platform-risc0`]
  - [`ere-platform-sp1`]
  - [`ere-platform-ziren`]
  - [`ere-platform-zisk`]
- [`ere-dockerized`] - Docker wrapper implementation for [`ere-zkvm-interface`] of all zkVMs
- [`ere-io`] - Serialization utilities for host/guest IO communication
- Internal crates
  - [`ere-compiler`] - Cli to run `Compiler` used by [`ere-dockerized`]
  - [`ere-server`] - Server and client for `zkVM` operations used by [`ere-dockerized`]
  - [`ere-build-utils`] - Build-time utilities
  - [`ere-compile-utils`] - Compilation utilities
  - [`ere-test-utils`] - Testing utilities

[`ere-zkvm-interface`]: https://github.com/eth-act/ere/tree/master/crates/zkvm-interface
[`ere-platform-trait`]: https://github.com/eth-act/ere/tree/master/crates/zkvm-interface/platform
[`ere-airbender`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/airbender
[`ere-platform-airbender`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/airbender/platform
[`ere-jolt`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/jolt
[`ere-platform-jolt`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/jolt/platform
[`ere-miden`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/miden
[`ere-nexus`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/nexus
[`ere-platform-nexus`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/nexus/platform
[`ere-openvm`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/openvm
[`ere-platform-openvm`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/openvm/platform
[`ere-pico`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/pico
[`ere-platform-pico`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/pico/platform
[`ere-risc0`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/risc0
[`ere-platform-risc0`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/risc0/platform
[`ere-sp1`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/sp1
[`ere-platform-sp1`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/sp1/platform
[`ere-ziren`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/ziren
[`ere-platform-ziren`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/ziren/platform
[`ere-zisk`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/zisk
[`ere-platform-zisk`]: https://github.com/eth-act/ere/tree/master/crates/zkvm/zisk/platform
[`ere-dockerized`]: https://github.com/eth-act/ere/tree/master/crates/dockerized
[`ere-compiler`]: https://github.com/eth-act/ere/tree/master/crates/dockerized/compiler
[`ere-server`]: https://github.com/eth-act/ere/tree/master/crates/dockerized/server
[`ere-io`]: https://github.com/eth-act/ere/tree/master/crates/io
[`ere-build-utils`]: https://github.com/eth-act/ere/tree/master/crates/build-utils
[`ere-compile-utils`]: https://github.com/eth-act/ere/tree/master/crates/compile-utils
[`ere-test-utils`]: https://github.com/eth-act/ere/tree/master/crates/test-utils

## Architecture

### The Interface

`ere-zkvm-interface` provides traits for host:

- `Compiler`

  Compile a guest program into a zkVM-specific artifact (typically a RISC-V ELF, or a wrapper with preprocessing data).

- `zkVM`

  Execute, prove and verify that artifact. A zkVM instance is created for specific artifact that comes from the `Compiler`.

`ere-platform-trait` provides traits for guest program:

- `Platform`

  Provides platform dependent methods for IO read/write and cycle tracking. It also re-exports the runtime SDK of the zkVM, that is guaranteed to match the same version with the zkVM host when `ere-{zkvm}` and `ere-platform-{zkvm}` have the same version.

### Communication between Host and Guest

Host and guest communicate through raw bytes. Serialization/deserialization can be done in any way as long as they agree with each other.

#### Reading Private Values from Host

The `Input` structure holds stdin as raw bytes. There are 2 ways to use it:

1. `Input::new().with_prefixed_stdin(data)` for `Platform::read_whole_input()`

    The `Platform` trait provides a unified interface to read the whole stdin. However, some zkVMs don't provide access to the stdin length, so we require it to have a length prefix.

    The method `Input::with_prefixed_stdin` automatically adds a LE u32 length prefix to the stdin. In the guest, `Platform::read_whole_input` will return only the actual data.

    Without the length prefix, the `Platform::read_whole_input` will cause guest panic at runtime.

2. `Input::new().with_stdin(data)` for zkVM-specific stdin APIs

    The method `Input::with_stdin` sets stdin without modification. Use this when you need direct access to zkVM-specific stdin APIs (e.g., `sp1_zkvm::io::read`, `risc0_zkvm::guest::env::read`), such as streaming reads or partial data consumption.

#### Writing Public Values to Host

Public values written in the guest program (via `Platform::write_whole_output()` or zkVM-specific output APIs) are returned as raw bytes to the host after `zkVM::execute`, `zkVM::prove` and `zkVM::verify` methods.

Different zkVMs handles public values in different approaches:

| zkVM      | Size Limit                | Note                            |
| --------- | ------------------------- | ------------------------------- |
| Airbender | 32 bytes                  | Padded to 32 bytes with zeros   |
| Jolt      | 4096 bytes (Configurable) |                                 |
| Miden     | 16 words                  | Word = Goldilocks field element |
| Nexus     | unlimited                 | Size configured automatically   |
| OpenVM    | 32 bytes                  | Padded to 32 bytes with zeros   |
| Pico      | unlimited                 | Hashed internally               |
| Risc0     | unlimited                 | Hashed internally               |
| SP1       | unlimited                 | Hashed internally               |
| Ziren     | unlimited                 | Hashed internally               |
| Zisk      | 256 bytes                 |                                 |

For zkVMs with size limits on public values, `OutputHashedPlatform<P, D>` serves as a wrapper that hashes outputs before calling the inner `P::write_whole_output`. This enables the same guest program to run across all zkVMs regardless of their size constraints:

```rust
OutputHashedPlatform::<OpenVMPlatform, Sha256>::write_whole_output(&large_output);
```

## Supported zkVMs

| zkVM      | Version                                                                | GPU |
| --------- | ---------------------------------------------------------------------- | --- |
| Airbender | [`0.5.1`](https://github.com/matter-labs/zksync-airbender/tree/v0.5.1) | Yes |
| Jolt      | [`0.3.0-alpha`](https://github.com/a16z/jolt/tree/v0.3.0-alpha)        | No  |
| Miden     | [`0.19.1`](https://github.com/0xMiden/miden-vm/tree/v0.19.1)           | No  |
| Nexus     | [`0.3.5`](https://github.com/nexus-xyz/nexus-zkvm/tree/v0.3.5)         | No  |
| OpenVM    | [`1.4.1`](https://github.com/openvm-org/openvm/tree/v1.4.1)            | Yes |
| Pico      | [`1.1.8`](https://github.com/brevis-network/pico/tree/v1.1.8)          | No  |
| Risc0     | [`3.0.4`](https://github.com/risc0/risc0/tree/v3.0.4)                  | Yes |
| SP1       | [`5.2.3`](https://github.com/succinctlabs/sp1/tree/v5.2.3)             | Yes |
| Ziren     | [`1.2.2`](https://github.com/ProjectZKM/Ziren/tree/v1.2.2)             | No  |
| Zisk      | [`0.13.0`](https://github.com/0xPolygonHermez/zisk/tree/v0.13.0)       | Yes |

## Examples

### With SDK Installation

Install the required zkVM SDKs locally for better performance and debugging.

#### 1. Install SDKs

Install the SP1 SDK as an example

```bash
bash scripts/sdk_installers/install_sp1_sdk.sh
```

#### 2. Create Guest Program

```toml
# guest/Cargo.toml

[workspace]

[package]
name = "guest"
edition = "2024"

[dependencies]
ere-platform-sp1 = { git = "https://github.com/eth-act/ere.git" }
```

```rust
// guest/src/main.rs

#![no_main]

use ere_platform_sp1::{sp1_zkvm, Platform, SP1Platform};

sp1_zkvm::entrypoint!(main);

type P = SP1Platform;

pub fn main() {
    // Read serialized input and deserialize it.
    let input = P::read_whole_input();
    let n = u64::from_le_bytes(input.as_slice().try_into().unwrap());

    // Compute nth fib.
    let fib_n = fib(n);

    // Write serialized output.
    let output = [input, fib_n.to_le_bytes().to_vec()].concat();
    P::write_whole_output(&output);
}

fn fib(n: u64) -> u64 {
    let mut a = 0;
    let mut b = 1;
    for _ in 0..n {
        let c = a + b;
        a = b;
        b = c;
    }
    a
}
```

#### 3. Create Host

```toml
# host/Cargo.toml

[workspace]

[package]
name = "host"
edition = "2024"

[dependencies]
ere-zkvm-interface = { git = "https://github.com/eth-act/ere.git" }
ere-sp1 = { git = "https://github.com/eth-act/ere.git" }
```

```rust
// host/src/main.rs

use ere_sp1::{compiler::RustRv32imaCustomized, zkvm::EreSP1};
use ere_zkvm_interface::{
    compiler::Compiler,
    zkvm::{Input, ProofKind, ProverResourceType, zkVM},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guest_directory = Path::new("path/to/guest");

    // Compile guest program with SP1 customized toolchain
    let compiler = RustRv32imaCustomized;
    let program = compiler.compile(guest_directory)?;

    // Create zkVM instance (setup/preprocessing happens here)
    let zkvm = EreSP1::new(program, ProverResourceType::Cpu)?;

    // Prepare input
    // Use `with_prefixed_stdin` when guest uses `Platform::read_whole_input()`
    let input = Input::new().with_prefixed_stdin(10u64.to_le_bytes().to_vec());
    let expected_output = [input, 55u64.to_le_bytes()].concat();

    // Execute
    let (public_values, report) = zkvm.execute(&input)?;
    assert_eq!(public_values, expected_output);
    println!("Execution cycles: {}", report.total_num_cycles);

    // Prove
    let (public_values, proof, report) = zkvm.prove(&input, ProofKind::default())?;
    assert_eq!(public_values, expected_output);
    println!("Proving time: {:?}", report.proving_time);

    // Verify
    let public_values = zkvm.verify(&proof)?;
    assert_eq!(public_values, expected_output);
    println!("Proof verified successfully!");

    Ok(())
}
```

### Docker-Only Setup

Use Docker for zkVM operations without installing SDKs locally. Only requires Docker to be installed.

#### 1. Create Guest Program

We use the same guest program created above.

#### 2. Create Host

```toml
# host/Cargo.toml

[workspace]

[package]
name = "host"
edition = "2024"

[dependencies]
ere-zkvm-interface = { git = "https://github.com/eth-act/ere.git" }
ere-dockerized = { git = "https://github.com/eth-act/ere.git" }
```

```rust
// host/src/main.rs

use ere_dockerized::{CompilerKind, DockerizedCompiler, DockerizedzkVM, zkVMKind};
use ere_zkvm_interface::{
    compiler::Compiler,
    zkvm::{Input, ProofKind, ProverResourceType, zkVM},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guest_directory = Path::new("path/to/guest");

    // Compile guest program with SP1 customized toolchain (builds Docker images if needed)
    let compiler =
        DockerizedCompiler::new(zkVMKind::SP1, CompilerKind::RustCustomized, guest_directory)?;
    let program = compiler.compile(guest_directory)?;

    // Create zkVM instance (builds Docker images if needed)
    // It spawns a container that runs a gRPC server handling zkVM operations
    let zkvm = DockerizedzkVM::new(zkVMKind::SP1, program, ProverResourceType::Cpu)?;

    // Prepare input
    // Use `with_prefixed_stdin` when guest uses `Platform::read_whole_input()`
    let input = Input::new().with_prefixed_stdin(10u64.to_le_bytes().to_vec());
    let expected_output = [input, 55u64.to_le_bytes()].concat();

    // Execute
    let (public_values, report) = zkvm.execute(&input)?;
    assert_eq!(public_values, expected_output);
    println!("Execution cycles: {}", report.total_num_cycles);

    // Prove
    let (public_values, proof, report) = zkvm.prove(&input, ProofKind::default())?;
    assert_eq!(public_values, expected_output);
    println!("Proving time: {:?}", report.proving_time);

    // Verify
    let public_values = zkvm.verify(&proof)?;
    assert_eq!(public_values, expected_output);
    println!("Proof verified successfully!");

    Ok(())
}
```

## Directory Layout

```
ere/
├── crates/                        # Rust crates
│   ├── zkvm-interface/            # ere-zkvm-interface
│   │   └── platform/              # ere-platform-trait
│   ├── zkvm/
│   │   └── {zkvm}/                # ere-{zkvm}
│   │       └── platform/          # ere-platform-{zkvm}
│   ├── dockerized/                # ere-dockerized
│   │   ├── compiler/              # ere-compiler
│   │   └── server/                # ere-server
│   ├── io/                        # ere-io
│   ├── build-utils/               # ere-build-utils
│   ├── compile-utils/             # ere-compile-utils
│   └── test-utils/                # ere-test-utils
│
├── docker/                        # Dockerfile used by ere-dockerized
│   ├── Dockerfile.base            # ere-base
│   └── {zkvm}/
│       ├── Dockerfile.base        # ere-base-{zkvm}
│       ├── Dockerfile.compiler    # ere-compiler-{zkvm}
│       └── Dockerfile.server      # ere-server-{zkvm}
│
├── scripts/                       # SDK installation scripts per zkVM
└── tests/                         # Guest programs per zkVM for integration test
```

## Contributing

PRs and issues are welcome!

## Disclaimer

zkVMs evolve quickly; expect breaking changes. Although the API is generic, its primary target is **zkEVMs**, which may for example, guide the default set of precompiles.

## License

Licensed under either of

* MIT license (LICENSE‑MIT or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* Apache License, Version 2.0 (LICENSE‑APACHE or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.

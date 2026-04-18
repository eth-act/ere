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
- [Environment Variables](#environment-variables)
- [Directory Layout](#directory-layout)
- [Contributing](#contributing)
- [Disclaimer](#disclaimer)
- [License](#license)

## Supported Rust Versions (MSRV)

The current MSRV (minimum supported rust version) is 1.88.

## Overview

This repository contains the following crates:

- Traits
  - [`ere-compiler-core`] - `Compiler` trait and `Elf` type for compiling guest programs
  - [`ere-prover-core`] - `zkVMProver` trait, `Input`, `ProverResource`, and execution/proving reports
  - [`ere-platform-core`] - `Platform` trait for guest program
  - [`ere-verifier-core`] - `zkVMVerifier` trait and `PublicValues`
- Per-zkVM implementations for [`ere-compiler-core`] (host)
  - [`ere-compiler-airbender`]
  - [`ere-compiler-openvm`]
  - [`ere-compiler-risc0`]
  - [`ere-compiler-sp1`]
  - [`ere-compiler-zisk`]
- Per-zkVM implementations for [`ere-prover-core`] (host)
  - [`ere-prover-airbender`]
  - [`ere-prover-openvm`]
  - [`ere-prover-risc0`]
  - [`ere-prover-sp1`]
  - [`ere-prover-zisk`]
- Per-zkVM implementations for [`ere-platform-core`] (guest)
  - [`ere-platform-airbender`]
  - [`ere-platform-openvm`]
  - [`ere-platform-risc0`]
  - [`ere-platform-sp1`]
  - [`ere-platform-zisk`]
- Per-zkVM implementations for [`ere-verifier-core`] (lightweight host verifier)
  - [`ere-verifier-airbender`]
  - [`ere-verifier-openvm`]
  - [`ere-verifier-risc0`]
  - [`ere-verifier-sp1`]
  - [`ere-verifier-zisk`]
- [`ere-dockerized`] - Docker wrapper that spawns [`ere-server`] containers to run zkVM operations without local SDK installation
- [`ere-cluster-client-zisk`] - ZisK distributed-cluster client used by [`ere-prover-zisk`] when `ProverResource::Cluster` is selected
- [`ere-codec`] - Canonical byte codec (`Encode`/`Decode` + macros) shared across verifier crates
- [`ere-catalog`] - Catalog of supported zkVMs and compilers (`zkVMKind`, `CompilerKind`, SDK versions, Docker image tag)
- Internal crates
  - [`ere-compiler`] - CLI binary to run `Compiler` used by [`ere-dockerized`]
  - [`ere-server`] - Server binary that exposes `zkVMProver` operations over gRPC (also provides a `keygen` subcommand)
  - [`ere-server-api`] - gRPC wire contract (`proto/api.proto` and generated prost/twirp types) shared by [`ere-server`] and [`ere-server-client`]
  - [`ere-server-client`] - Client library for [`ere-server`], used by [`ere-dockerized`]
  - [`ere-util-build`] - Build-time utilities (SDK version + Docker image tag detection)
  - [`ere-util-compile`] - Cross-compilation utilities (`CargoBuildCmd`, `RustTarget`, toolchain management)
  - [`ere-util-test`] - Testing utilities (`Program`, `TestCase`, `BasicProgram`, codec markers)

[`ere-compiler-core`]: https://github.com/eth-act/ere/tree/master/crates/compiler/core
[`ere-prover-core`]: https://github.com/eth-act/ere/tree/master/crates/prover/core
[`ere-platform-core`]: https://github.com/eth-act/ere/tree/master/crates/platform/core
[`ere-verifier-core`]: https://github.com/eth-act/ere/tree/master/crates/verifier/core
[`ere-compiler-airbender`]: https://github.com/eth-act/ere/tree/master/crates/compiler/airbender
[`ere-compiler-openvm`]: https://github.com/eth-act/ere/tree/master/crates/compiler/openvm
[`ere-compiler-risc0`]: https://github.com/eth-act/ere/tree/master/crates/compiler/risc0
[`ere-compiler-sp1`]: https://github.com/eth-act/ere/tree/master/crates/compiler/sp1
[`ere-compiler-zisk`]: https://github.com/eth-act/ere/tree/master/crates/compiler/zisk
[`ere-cluster-client-zisk`]: https://github.com/eth-act/ere/tree/master/crates/cluster-client/zisk
[`ere-prover-airbender`]: https://github.com/eth-act/ere/tree/master/crates/prover/airbender
[`ere-platform-airbender`]: https://github.com/eth-act/ere/tree/master/crates/platform/airbender
[`ere-verifier-airbender`]: https://github.com/eth-act/ere/tree/master/crates/verifier/airbender
[`ere-prover-openvm`]: https://github.com/eth-act/ere/tree/master/crates/prover/openvm
[`ere-platform-openvm`]: https://github.com/eth-act/ere/tree/master/crates/platform/openvm
[`ere-verifier-openvm`]: https://github.com/eth-act/ere/tree/master/crates/verifier/openvm
[`ere-prover-risc0`]: https://github.com/eth-act/ere/tree/master/crates/prover/risc0
[`ere-platform-risc0`]: https://github.com/eth-act/ere/tree/master/crates/platform/risc0
[`ere-verifier-risc0`]: https://github.com/eth-act/ere/tree/master/crates/verifier/risc0
[`ere-prover-sp1`]: https://github.com/eth-act/ere/tree/master/crates/prover/sp1
[`ere-platform-sp1`]: https://github.com/eth-act/ere/tree/master/crates/platform/sp1
[`ere-verifier-sp1`]: https://github.com/eth-act/ere/tree/master/crates/verifier/sp1
[`ere-prover-zisk`]: https://github.com/eth-act/ere/tree/master/crates/prover/zisk
[`ere-platform-zisk`]: https://github.com/eth-act/ere/tree/master/crates/platform/zisk
[`ere-verifier-zisk`]: https://github.com/eth-act/ere/tree/master/crates/verifier/zisk
[`ere-dockerized`]: https://github.com/eth-act/ere/tree/master/crates/dockerized
[`ere-compiler`]: https://github.com/eth-act/ere/tree/master/crates/compiler/cli
[`ere-server`]: https://github.com/eth-act/ere/tree/master/crates/server/cli
[`ere-server-api`]: https://github.com/eth-act/ere/tree/master/crates/server/api
[`ere-server-client`]: https://github.com/eth-act/ere/tree/master/crates/server/client
[`ere-codec`]: https://github.com/eth-act/ere/tree/master/crates/codec
[`ere-catalog`]: https://github.com/eth-act/ere/tree/master/crates/catalog
[`ere-util-build`]: https://github.com/eth-act/ere/tree/master/crates/util/build
[`ere-util-compile`]: https://github.com/eth-act/ere/tree/master/crates/util/compile
[`ere-util-test`]: https://github.com/eth-act/ere/tree/master/crates/util/test

## Architecture

### The Interface

Host-side traits:

- `Compiler` (from `ere-compiler-core`)

  Compile a guest program into an `Elf`.

- `zkVMProver` (from `ere-prover-core`)

  Execute, prove and verify. A zkVM instance is created for an `Elf` produced by a `Compiler`; setup/preprocessing happens in the constructor.

- `zkVMVerifier` (from `ere-verifier-core`)

  Lightweight verifier that accepts a `{Name}ProgramVk` + `{Name}Proof` and returns `PublicValues`. Pulled in standalone by verify-only consumers without the prover deps.

Guest-side trait (`ere-platform-core`):

- `Platform`

  Provides platform-dependent methods for IO read/write and cycle tracking. It also re-exports the runtime SDK of the zkVM, guaranteed to match the host when `ere-prover-{zkvm}` and `ere-platform-{zkvm}` share the same version.

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

Public values written in the guest program (via `Platform::write_whole_output()` or zkVM-specific output APIs) are returned as raw bytes to the host after `zkVMProver::execute`, `zkVMProver::prove` and `zkVMProver::verify` methods.

Different zkVMs handles public values in different approaches:

| zkVM      | Size Limit | Note                          |
| --------- | ---------- | ----------------------------- |
| Airbender | 32 bytes   | Padded to 32 bytes with zeros |
| OpenVM    | 32 bytes   | Padded to 32 bytes with zeros |
| Risc0     | unlimited  | Hashed internally             |
| SP1       | unlimited  | Hashed internally             |
| Zisk      | 256 bytes  |                               |

## Supported zkVMs

| zkVM      | Version                                                                | ISA       |  GPU  | Multi GPU | Cluster |
| --------- | ---------------------------------------------------------------------- | --------- | :---: | :-------: | :-----: |
| Airbender | [`0.5.2`](https://github.com/matter-labs/zksync-airbender/tree/v0.5.2) | `RV32IMA` |   V   |     V     |         |
| OpenVM    | [`1.4.3`](https://github.com/openvm-org/openvm/tree/v1.4.3)            | `RV32IMA` |   V   |           |         |
| Risc0     | [`3.0.5`](https://github.com/risc0/risc0/tree/v3.0.5)                  | `RV32IMA` |   V   |     V     |         |
| SP1       | [`6.0.1`](https://github.com/succinctlabs/sp1/tree/v6.0.1)             | `RV64IMA` |   V   |           |         |
| Zisk      | [`0.16.1`](https://github.com/0xPolygonHermez/zisk/tree/v0.16.1)       | `RV64IMA` |   V   |     V     |    V    |

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
ere-prover-core = { git = "https://github.com/eth-act/ere.git" }
ere-prover-sp1 = { git = "https://github.com/eth-act/ere.git" }
```

```rust
// host/src/main.rs

use ere_compiler_core::Compiler;
use ere_compiler_sp1::SP1RustRv64imaCustomized;
use ere_prover_core::{Input, ProverResource, zkVMProver};
use ere_prover_sp1::SP1Prover;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guest_directory = Path::new("path/to/guest");

    // Compile guest program with SP1 customized toolchain
    let compiler = SP1RustRv64imaCustomized;
    let elf = compiler.compile(guest_directory)?;

    // Create zkVM instance (setup/preprocessing happens here)
    let zkvm = SP1Prover::new(elf, ProverResource::Cpu)?;

    // Prepare input
    // Use `with_prefixed_stdin` when guest uses `Platform::read_whole_input()`
    let input = Input::new().with_prefixed_stdin(10u64.to_le_bytes().to_vec());
    let expected_output = [input, 55u64.to_le_bytes()].concat();

    // Execute
    let (public_values, report) = zkvm.execute(&input)?;
    assert_eq!(public_values, expected_output);
    println!("Execution cycles: {}", report.total_num_cycles);

    // Prove
    let (public_values, proof, report) = zkvm.prove(&input)?;
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
ere-prover-core = { git = "https://github.com/eth-act/ere.git" }
ere-dockerized = { git = "https://github.com/eth-act/ere.git" }
```

```rust
// host/src/main.rs

use ere_compiler_core::Compiler;
use ere_dockerized::{
    CompilerKind, DockerizedCompiler, DockerizedzkVM, DockerizedzkVMConfig, zkVMKind,
};
use ere_prover_core::{Input, ProverResource, zkVMProver};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guest_directory = Path::new("path/to/guest");

    // Compile guest program with SP1 customized toolchain (builds Docker images if needed)
    let compiler =
        DockerizedCompiler::new(zkVMKind::SP1, CompilerKind::RustCustomized, guest_directory)?;
    let elf = compiler.compile(guest_directory)?;

    // Create zkVM instance (builds Docker images if needed)
    // It spawns a container that runs a gRPC server handling zkVM operations
    let zkvm = DockerizedzkVM::new(
        zkVMKind::SP1,
        elf,
        ProverResource::Cpu,
        DockerizedzkVMConfig::default(),
    )?;

    // Prepare input
    // Use `with_prefixed_stdin` when guest uses `Platform::read_whole_input()`
    let input = Input::new().with_prefixed_stdin(10u64.to_le_bytes().to_vec());
    let expected_output = [input, 55u64.to_le_bytes()].concat();

    // Execute
    let (public_values, report) = zkvm.execute(&input)?;
    assert_eq!(public_values, expected_output);
    println!("Execution cycles: {}", report.total_num_cycles);

    // Prove
    let (public_values, proof, report) = zkvm.prove(&input)?;
    assert_eq!(public_values, expected_output);
    println!("Proving time: {:?}", report.proving_time);

    // Verify
    let public_values = zkvm.verify(&proof)?;
    assert_eq!(public_values, expected_output);
    println!("Proof verified successfully!");

    Ok(())
}
```

## Environment Variables

| Variable                         | Description                                                                                                                             | Default |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| `ERE_IMAGE_REGISTRY`             | Specifies docker image registry of the images. When specified, it will try to pull image from the registry and possibly skip building.  | ``      |
| `ERE_FORCE_REBUILD_DOCKER_IMAGE` | Force to rebuild docker images locally even they exist, it also prevents pulling image from registry.                                   | `false` |
| `ERE_GPU_DEVICES`                | Specifies which GPU devices to use when running Docker containers for GPU-enabled zkVMs. The value is passed to Docker's `--gpus` flag. | `all`   |
| `ERE_DOCKER_NETWORK`             | Specifies the Docker network being used (if any) so spawned `ere-server-*` containers will join that network.                           | ``      |

Example usage:

```bash
# Use all GPUs (default)
ere prove ...

# Use specific GPU devices
ERE_GPU_DEVICES="device=0" ere prove ...

# Use multiple specific GPUs
ERE_GPU_DEVICES="device=0,1" ere prove ...

# Can also signal to use any available GPUs
ERE_GPU_DEVICES="4" ere prove ...
```

## Directory Layout

```
ere/
├── crates/                        # Rust crates
│   ├── prover/
│   │   ├── core/                  # ere-prover-core
│   │   └── {zkvm}/                # ere-prover-{zkvm}
│   ├── platform/
│   │   ├── core/                  # ere-platform-core
│   │   └── {zkvm}/                # ere-platform-{zkvm}
│   ├── verifier/
│   │   ├── core/                  # ere-verifier-core
│   │   └── {zkvm}/                # ere-verifier-{zkvm}
│   ├── dockerized/                # ere-dockerized
│   ├── compiler/
│   │   ├── cli/                   # ere-compiler
│   │   ├── core/                  # ere-compiler-core
│   │   └── {zkvm}/                # ere-compiler-{zkvm}
│   ├── server/
│   │   ├── api/                   # ere-server-api
│   │   ├── cli/                   # ere-server
│   │   └── client/                # ere-server-client
│   ├── cluster-client/
│   │   └── zisk/                  # ere-cluster-client-zisk
│   ├── catalog/                   # ere-catalog
│   ├── codec/                     # ere-codec
│   └── util/
│       ├── build/                 # ere-util-build
│       ├── compile/               # ere-util-compile
│       └── test/                  # ere-util-test
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

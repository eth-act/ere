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

* [Features](#features)
* [Supported zkVMs](#supported-zkvms)
* [Quick Start](#quick-start)

  * [Option 1: With SDK Installation](#option-1-with-sdk-installation)
  * [Option 2: Docker-Only Setup](#option-2-docker-only-setup)
* [Directory Layout](#directory-layout)
* [Architecture](#architecture)

  * [The Interface](#the-interface)
  * [Backend Crates](#backend-crates)
  * [Input Handling](#input-handling)
* [Contributing](#contributing)
* [Disclaimer](#disclaimer)
* [License](#license)

## Features

* **Unified Rust API** for compiling, executing, proving & verifying zkVM programs
* **Pluggable back‑ends** – easily switch between different zkVMs
* **SDK bootstrap scripts** for every supported zkVM
* **End‑to‑end test suite** covering compilation → proof → verification for each backend

## Supported zkVMs

- SP1
- OpenVM
- Risc Zero
- Jolt
- Pico
- Zisk
- Nexus

## Quick Start

This guide assumes you have Rust and Cargo installed. If not, please refer to the [Rust installation guide](https://www.rust-lang.org/tools/install).
Choose your setup approach:

### Option 1: With SDK Installation

Install the required zkVM SDKs locally for better performance and debugging.

#### 1. Install SDKs

```bash
bash scripts/sdk_installers/install_sp1_sdk.sh
```

#### 2. Add Dependencies

```toml
# Cargo.toml
[dependencies]
zkvm-interface = { git = "https://github.com/eth-act/ere.git", tag = "v0.0.12" }
ere-sp1        = { git = "https://github.com/eth-act/ere.git", tag = "v0.0.12" }
```

#### 3. Compile & Prove Example

```rust
// main.rs
use ere_sp1::{EreSP1, RV32_IM_SUCCINCT_ZKVM_ELF};
use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guest_directory = std::path::Path::new("workspace/guest");

    // Compile guest
    let compiler = RV32_IM_SUCCINCT_ZKVM_ELF;
    let program = compiler.compile(guest_directory)?;

    // Create zkVM instance
    let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

    // Prepare inputs
    let mut io = Input::new();
    io.write(42u32);

    // Execute
    let _report = zkvm.execute(&io)?;

    // Prove
    let (proof, _report) = zkvm.prove(&io)?;

    // Verify
    zkvm.verify(&proof)?;

    Ok(())
}
```

### Option 2: Docker-Only Setup

Use Docker for zkVM operations without installing SDKs locally. Only requires Docker to be installed.

#### 1. Add Dependencies

```toml
# Cargo.toml
[dependencies]
zkvm-interface = { git = "https://github.com/eth-act/ere.git", tag = "v0.0.12" }
ere-dockerized = { git = "https://github.com/eth-act/ere.git", tag = "v0.0.12" }
```

#### 2. Compile & Prove Example

```rust
// main.rs
use ere_dockerized::{EreDockerizedCompiler, EreDockerizedzkVM, ErezkVM};
use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guest_directory = std::path::Path::new("workspace/guest");

    // Compile guest
    let compiler = EreDockerizedCompiler::new(ErezkVM::SP1, std::path::Path::new("workspace"));
    let program = compiler.compile(guest_directory)?;

    // Create zkVM instance
    let zkvm = EreDockerizedzkVM::new(ErezkVM::SP1, program, ProverResourceType::Cpu)?;

    // Prepare inputs
    let mut io = Input::new();
    io.write(42u32);

    // Execute
    let _report = zkvm.execute(&io)?;

    // Prove
    let (proof, _report) = zkvm.prove(&io)?;

    // Verify
    zkvm.verify(&proof)?;

    Ok(())
}
```

## Directory Layout

```
crates/
  zkvm-interface/     ← core traits & types
  ere-{backend}/      ← backend adapters (sp1, openvm, …)
tests/                ← guest programs & integration tests
scripts/sdk_installers/ ← SDK install helpers
docker/               ← Dockerfiles & build contexts
```

## Architecture

### The Interface

`zkvm-interface` exposes two core traits:

* **Compiler** – compile a guest project into the correct zkVM artifact. For most this will be a RISCV ELF binary or some type that wraps it and includes extra metadata such as a proving and verifying key.
* **zkVM** – execute, prove & verify that artifact. A zkVM instance is created for specific `program`, where the `program` comes from the `Compiler`.

### Backend Crates

Each `ere-{backend}` crate implements the above traits for its zkVM.

### Input Handling

The `Input` type supports both chunked (`Vec<Vec<u8>>`) and contiguous (`Vec<u8>`) modes to satisfy differing backend APIs.

## Contributing

PRs and issues are welcome!

## Disclaimer

zkVMs evolve quickly; expect breaking changes. Although the API is generic, its primary target is **zkEVMs**, which may for example, guide the default set of precompiles.

## License

Licensed under either of

* MIT license (LICENSE‑MIT or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* Apache License, Version 2.0 (LICENSE‑APACHE or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.

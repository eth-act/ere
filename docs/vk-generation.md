# Verification Key Generation

Ere checks a proof against a program verifying key that identifies one specific compiled guest ELF. `Verifier::new` in `ere-verifier` takes that key as an opaque byte string and decodes it with the codec of the selected zkVM, so a key file is only accepted when it already matches that exact wire encoding.

This guide records the encoding for each supported zkVM and gives the commands that produce a matching key file from the upstream zkVM toolchain alone. A guest provider can therefore run the upstream tool over their own program and hand over a key file that Ere takes as is.

## OpenVM

### Format

`bitcode` encoding of the upstream `VerificationBaseline`.

### Instructions

`cargo openvm commit` has no flag for a compiled ELF and works off a Rust project it builds itself, so the key comes from the rust script below instead. The script applies the same app config `ere-prover-openvm` builds its prover with and generates the aggregation keys in process, so no OpenVM installation is involved.

1. Setup

    ```bash
    sudo apt-get update && sudo apt-get install -y curl ca-certificates git build-essential
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
    . "$HOME/.cargo/env"
    ```
2. Save the script as `openvm_vk_gen.rs`

    ```rust
    #!/usr/bin/env -S cargo +nightly -Zscript

    ---
    [package]
    edition = "2024"

    [profile.dev]
    opt-level = 3

    [dependencies]
    bitcode = { version = "0.6", features = ["serde"] }
    openvm-sdk = { git = "https://github.com/openvm-org/openvm.git", tag = "v2.0.0" }
    openvm-sdk-config = { git = "https://github.com/openvm-org/openvm.git", tag = "v2.0.0" }
    openvm-stark-sdk = { git = "https://github.com/openvm-org/stark-backend.git", tag = "v2.0.0" }
    ---

    use std::{env, error::Error, fs};
    use openvm_sdk::{Sdk, config::AppConfig};
    use openvm_sdk_config::SdkVmConfig;
    use openvm_stark_sdk::config::{MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security};

    fn main() -> Result<(), Box<dyn Error>> {
        let mut args = env::args().skip(1);
        let elf_path = args.next().expect("usage: <elf-path> <vk-path>");
        let vk_path = args.next().expect("usage: <elf-path> <vk-path>");

        let app_config = {
            let mut config = SdkVmConfig::standard();
            config.system.config = config.system.config.with_public_values(256);
            let system_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
            AppConfig::new(config.optimize(), system_params)
        };
        let sdk = Sdk::builder()
            .app_config(app_config)
            .agg_params(Default::default())
            .build()?;
        let baseline = sdk.prover(fs::read(elf_path)?)?.generate_baseline();

        fs::write(vk_path, bitcode::serialize(&baseline)?)?;
        Ok(())
    }
    ```
3. Generate VK

    ```bash
    GUEST=<guest>
    ELF_PATH=<elf-path>
    cargo +nightly -Zscript openvm_vk_gen.rs $ELF_PATH stateless-validator-$GUEST-openvm.vk
    ```
4. Sanity check

    ```bash
    printf 'Generated OpenVM VK sha256: %s\n' "$(sha256sum stateless-validator-$GUEST-openvm.vk | cut -d' ' -f1)"
    ```

## SP1

### Format

32-bytes holding the 8 koalabear field elements of the digest packed as base-2^31 digits of a big-endian integer, which is the form [`HashableKey::bytes32`](https://github.com/succinctlabs/sp1/blob/v6.3.1/crates/hypercube/src/verifier/hashable_key.rs) prints and what `cargo prove vkey` reports.

### Instructions

1. Setup

    ```bash
    sudo apt-get update && sudo apt-get install -y curl ca-certificates git
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
    curl -L https://sp1up.succinct.xyz | bash
    export PATH="$HOME/.sp1/bin:$PATH"
    sp1up -v v6.3.1
    ```
2. Generate VK

    ```bash
    GUEST=<guest>
    ELF_PATH=<elf-path>
    cargo prove vkey --elf $ELF_PATH | grep -oE '0x[0-9a-f]{64}' | cut -c3- | tr a-f A-F | basenc --base16 -d > stateless-validator-$GUEST-sp1.vk
    ```
3. Sanity check

    ```bash
    printf 'Generated SP1 VK: 0x%s\n' "$(od -An -v -tx1 stateless-validator-$GUEST-sp1.vk | tr -d ' \n')"
    ```

## ZisK

### Format

32-bytes containing 4 goldilocks field element canonical bytes in little-endian, holding the Merkle root of the compiled guest ROM trace.

### Instructions

1. Setup

    ```bash
    sudo apt-get update && sudo apt-get install -y xz-utils jq curl build-essential qemu-system libomp-dev libgmp-dev nlohmann-json3-dev protobuf-compiler uuid-dev libgrpc++-dev libsecp256k1-dev libsodium-dev libpqxx-dev nasm libopenmpi-dev openmpi-bin openmpi-common libclang-dev clang gcc-riscv64-unknown-elf python3
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
    export ZISK_VERSION=1.0.0-alpha SETUP_KEY=proving-no-consttree
    curl -sSf "https://raw.githubusercontent.com/0xPolygonHermez/zisk/v$ZISK_VERSION/ziskup/ziskup" | bash
    export PATH="$HOME/.zisk/bin:$PATH"
    ```
2. Generate VK

    ```bash
    GUEST=<guest>
    ELF_PATH=<elf-path>
    cargo-zisk-dev program-setup --elf $ELF_PATH --output-dir $(mktemp -d) 2>&1 \
      | grep -oP 'Root hash: \[\K[^]]+' \
      | python3 -c 'import sys; sys.stdout.buffer.write(b"".join(int(word).to_bytes(8, "little") for word in sys.stdin.read().split(",")))' \
      > stateless-validator-$GUEST-zisk.vk
    ```
3. Sanity check

    ```bash
    printf 'Generated ZisK VK: 0x%s\n' "$(od -An -v -tx1 stateless-validator-$GUEST-zisk.vk | tr -d ' \n')"
    ```

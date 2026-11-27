# Verification Key Generation

Ere checks a proof against a program VK that identifies one specific compiled guest ELF. `Verifier::new` in `ere-verifier` takes that VK as an opaque byte string and decodes it with the codec of the selected zkVM, so a VK is only accepted when it already matches that exact wire encoding.

This guide records the encoding for each supported zkVM and gives the commands that produce a matching VK from the upstream zkVM toolchain alone, without an Ere checkout.

## LambdaVM

### Format

LambdaVM has no verifying key separate from the program: its verifier binds a proof to the keccak256 digest of the ELF and recomputes the program commitments from the ELF itself. So the VK is the guest ELF, prefixed with its length in bytes as a `u64` little endian. The VK size is the ELF size plus 8 bytes.

### Instructions

1. Setup

    Build the guest ELF with the customized compiler (`LambdaVMRustRv64imaCustomized`, or the `ere-compiler-lambdavm` image with `--compiler-kind rust-customized`). The VK only needs `coreutils`.

2. Generate VK

    ```bash
    set -euo pipefail
    GUEST_NAME="<guest-name>"
    ELF_PATH="<elf-path>"
    ZKVM_VERSION="v0.1.0"
    VK="stateless-validator-$GUEST_NAME-lambdavm-$ZKVM_VERSION.vk"
    printf '%016X' "$(stat -c%s "$ELF_PATH")" | fold -w2 | tac | tr -d '\n' | basenc --base16 -d > "$VK"
    cat "$ELF_PATH" >> "$VK"
    ```
3. Sanity check

    ```bash
    [ "$(stat -c%s "$VK")" -eq "$(( $(stat -c%s "$ELF_PATH") + 8 ))" ] || { echo "expected ELF size + 8 bytes, got $(stat -c%s "$VK")" >&2; exit 1; }
    tail -c +9 "$VK" | cmp -s - "$ELF_PATH" || { echo "VK does not end with the ELF" >&2; exit 1; }
    printf 'Generated LambdaVM VK: %s bytes\n' "$(stat -c%s "$VK")"
    ```

## OpenVM

### Format

32-bytes holding the 8 BabyBear elements of the `app_exe_commit` of the program, each as a canonical `u32` in little-endian.

### Instructions

`cargo openvm commit` cannot read an ELF, so a script computes the commit. It builds the OpenVM fork of ere, because the upstream tag depends on a deleted `stark-backend` branch.

1. Setup

    ```bash
    sudo apt-get update && sudo apt-get install -y curl ca-certificates git build-essential

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly-2026-07-01
    . "$HOME/.cargo/env"

    ZKVM_VERSION="v2.1.0-preview"
    cat > openvm_vk_gen.rs <<EOF
    ---
    [package]
    edition = "2024"

    [profile.dev]
    opt-level = 3

    [dependencies]
    openvm-sdk = { git = "https://github.com/han0110/openvm.git", branch = "patch/$ZKVM_VERSION" }
    openvm-sdk-config = { git = "https://github.com/han0110/openvm.git", branch = "patch/$ZKVM_VERSION" }
    openvm-stark-sdk = { git = "https://github.com/han0110/stark-backend.git", branch = "patch/$ZKVM_VERSION" }
    ---

    use std::{env, error::Error, fs};
    use openvm_sdk::{Sdk, config::AppConfig};
    use openvm_sdk_config::SdkVmConfig;
    use openvm_stark_sdk::{
        config::{MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security},
        openvm_stark_backend::p3_field::PrimeField32,
    };

    fn main() -> Result<(), Box<dyn Error>> {
        let mut args = env::args().skip(1);
        let elf_path = args.next().expect("usage: <elf-path> <vk-path>");
        let vk_path = args.next().expect("usage: <elf-path> <vk-path>");

        let app_config = {
            let mut config = SdkVmConfig::standard();
            config.system.config = config.system.config.with_public_values_bytes(256);
            let system_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
            AppConfig::new(config.optimize(), system_params)
        };
        let sdk = Sdk::builder()
            .app_config(app_config)
            .agg_params(Default::default())
            .build()?;
        let app_exe_commit = sdk.app_prover(fs::read(elf_path)?)?.app_exe_commit();

        let vk: Vec<u8> = app_exe_commit
            .iter()
            .flat_map(|element| element.as_canonical_u32().to_le_bytes())
            .collect();
        fs::write(vk_path, vk)?;
        Ok(())
    }
    EOF
    ```
2. Generate VK

    ```bash
    set -euo pipefail
    GUEST_NAME="<guest-name>"
    ELF_PATH="<elf-path>"
    VK="stateless-validator-$GUEST_NAME-openvm-$ZKVM_VERSION.vk"
    cargo +nightly-2026-07-01 -Zscript openvm_vk_gen.rs "$ELF_PATH" "$VK"
    ```
3. Sanity check

    ```bash
    [ "$(stat -c%s "$VK")" -eq 32 ] || { echo "expected 32 bytes, got $(stat -c%s "$VK")" >&2; exit 1; }
    printf 'Generated OpenVM VK: 0x%s\n' "$(od -An -v -tx1 "$VK" | tr -d ' \n')"
    ```

## Pico

### Format

100-bytes holding the 25 KoalaBear elements of the `vm_vk` of the program, each as a canonical `u32` in little-endian.

### Instructions

The `mle-ethproofs-vk-bundle` binary in the Pico repository builds the VK bundle of an ELF. The VK is its `vm_vk`, bytes 8 to 107 of the zstd body.

1. Setup

    ```bash
    sudo apt-get update && sudo apt-get install -y curl ca-certificates git build-essential pkg-config libssl-dev protobuf-compiler libprotobuf-dev zstd

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly-2026-09-27
    . "$HOME/.cargo/env"

    ZKVM_VERSION="v3.1.5"
    git clone https://github.com/brevis-network/pico && git -C pico checkout "$ZKVM_VERSION"
    ```
2. Generate VK

    ```bash
    set -euo pipefail
    GUEST_NAME="<guest-name>"
    ELF_PATH="$(realpath <elf-path>)"
    VK="$PWD/stateless-validator-$GUEST_NAME-pico-$ZKVM_VERSION.vk"
    TEMPDIR="$(mktemp -d)"
    (cd pico && cargo run --release -p distributed-vm --bin mle-ethproofs-vk-bundle -- --elf "$ELF_PATH" "$TEMPDIR/bundle")
    tail -c +13 "$TEMPDIR/bundle" | zstd -dc > "$TEMPDIR/body"
    dd if="$TEMPDIR/body" of="$VK" bs=1 skip=8 count=100 status=none
    rm -rf "$TEMPDIR"
    ```
3. Sanity check

    ```bash
    [ "$(stat -c%s "$VK")" -eq 100 ] || { echo "expected 100 bytes, got $(stat -c%s "$VK")" >&2; exit 1; }
    printf 'Generated Pico VK: 0x%s\n' "$(od -An -v -tx1 "$VK" | tr -d ' \n')"
    ```

## SP1

### Format

32-bytes holding the 8 koalabear field elements of the digest packed as base-2^31 digits of a big-endian integer, which is the form [`HashableKey::bytes32`](https://github.com/succinctlabs/sp1/blob/v6.10.0/crates/hypercube/src/verifier/hashable_key.rs) prints and what `cargo prove vkey` reports.

### Instructions

1. Setup

    ```bash
    sudo apt-get update && sudo apt-get install -y curl ca-certificates git

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"

    ZKVM_VERSION="v6.10.0"
    curl -L https://sp1up.succinct.xyz | bash
    export PATH="$HOME/.sp1/bin:$PATH"
    sp1up -v "$ZKVM_VERSION"
    ```
2. Generate VK

    ```bash
    set -euo pipefail
    GUEST_NAME="<guest-name>"
    ELF_PATH="<elf-path>"
    VK="stateless-validator-$GUEST_NAME-sp1-$ZKVM_VERSION.vk"
    cargo prove vkey --elf "$ELF_PATH" | grep -oE '0x[0-9a-f]{64}' | cut -c3- | tr a-f A-F | basenc --base16 -d > "$VK"
    ```
3. Sanity check

    ```bash
    [ "$(stat -c%s "$VK")" -eq 32 ] || { echo "expected 32 bytes, got $(stat -c%s "$VK")" >&2; exit 1; }
    printf 'Generated SP1 VK: 0x%s\n' "$(od -An -v -tx1 "$VK" | tr -d ' \n')"
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

    ZKVM_VERSION="v1.4.0-alpha"
    export ZISK_VERSION=${ZKVM_VERSION#v} SETUP_KEY=proving-no-consttree USE_GPU=false
    curl -sSf "https://raw.githubusercontent.com/0xPolygonHermez/zisk/$ZKVM_VERSION/ziskup/ziskup" | bash -s -- --blake3
    export PATH="$HOME/.zisk/bin:$PATH"
    ```
2. Generate VK

    ```bash
    set -eu
    GUEST_NAME="<guest-name>"
    ELF_PATH="<elf-path>"
    VK="stateless-validator-$GUEST_NAME-zisk-$ZKVM_VERSION.vk"
    TEMPDIR="$(mktemp -d)"
    cargo-zisk-dev program-setup --elf "$ELF_PATH" --output-dir "$TEMPDIR" 2>&1 \
      | grep -oP 'Root hash: \[\K[^]]+' \
      | python3 -c 'import sys; sys.stdout.buffer.write(b"".join(int(word).to_bytes(8, "little") for word in sys.stdin.read().split(",")))' \
      > "$VK"
    rm -rf "$TEMPDIR"
    ```
3. Sanity check

    ```bash
    [ "$(stat -c%s "$VK")" -eq 32 ] || { echo "expected 32 bytes, got $(stat -c%s "$VK")" >&2; exit 1; }
    printf 'Generated ZisK VK: 0x%s\n' "$(od -An -v -tx1 "$VK" | tr -d ' \n')"
    ```

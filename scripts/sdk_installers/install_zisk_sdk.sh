#!/bin/bash
set -e

# --- Utility functions (duplicated) ---
# Checks if a tool is installed and available in PATH.
is_tool_installed() {
    command -v "$1" &> /dev/null
}

# Ensures a tool is installed. Exits with an error if not.
ensure_tool_installed() {
    local tool_name="$1"
    local purpose_message="$2"
    if ! is_tool_installed "${tool_name}"; then
        echo "Error: Required tool '${tool_name}' could not be found." >&2
        if [ -n "${purpose_message}" ]; then
            echo "       It is needed ${purpose_message}." >&2
        fi
        echo "       Please install it first and ensure it is in your PATH." >&2
        exit 1
    fi
}
# --- End of Utility functions ---

echo "Installing ZisK Toolchain and SDK using ziskup (prebuilt binaries)..."

# Prerequisites for ziskup and ZisK (some of these are for the SDK itself beyond ziskup)
ensure_tool_installed "curl" "to download the ziskup installer"
ensure_tool_installed "bash" "to run the ziskup installer"
ensure_tool_installed "rustup" "for managing Rust toolchains (ZisK installs its own)"
ensure_tool_installed "cargo" "as cargo-zisk is a cargo subcommand"

# Step 1: Download and run the script that installs the ziskup binary itself.
# Export SETUP_KEY=proving to ensure no interactive options in `ziskup`.
export ZISK_VERSION="0.14.0"
export SETUP_KEY=${SETUP_KEY:=proving}
curl "https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh" | bash
unset SETUP_KEY

# Step 2: Ensure the installed cargo-zisk binary is in PATH for this script session.
export PATH="$PATH:$HOME/.zisk/bin"

# FIXME: Issue for tracking: https://github.com/eth-act/ere/issues/200.
if true; then
    WORKSPACE=$(mktemp -d)
    git clone https://github.com/han0110/zisk.git --depth 1 --branch patch/v0.14.0 "$WORKSPACE"
    cargo build --manifest-path "$WORKSPACE/Cargo.toml" --release
    cp "$WORKSPACE/target/release/cargo-zisk" "$HOME/.zisk/bin/cargo-zisk"
    cp "$WORKSPACE/target/release/libzisk_witness.so" "$HOME/.zisk/bin/libzisk_witness.so"
    rm -rf "$WORKSPACE"
fi

# Verify ZisK installation
echo "Verifying ZisK installation..."

echo "Checking for 'zisk' toolchain..."
if rustup toolchain list | grep -q "^zisk"; then
    echo "ZisK Rust toolchain found."
else
    echo "Error: ZisK Rust toolchain ('zisk') not found after installation!" >&2
    exit 1
fi

echo "Checking for cargo-zisk CLI tool..."
if cargo-zisk --version; then
    echo "cargo-zisk CLI tool verified successfully."
else
    echo "Error: 'cargo-zisk --version' failed." >&2
    exit 1
fi

# Step 3: Build cargo-zisk-cuda from source with `gpu` feature enabled
if [ -n "$CUDA" ]; then
    WORKSPACE=$(mktemp -d)
    # FIXME: Issue for tracking: https://github.com/eth-act/ere/issues/200.
    # git clone https://github.com/0xPolygonHermez/zisk.git --depth 1 --tag "v$ZISK_VERSION" "$WORKSPACE"
    git clone https://github.com/han0110/zisk.git --depth 1 --branch patch/v0.14.0 "$WORKSPACE"
    cargo build --manifest-path "$WORKSPACE/Cargo.toml" --release --features gpu
    cp "$WORKSPACE/target/release/cargo-zisk" "$HOME/.zisk/bin/cargo-zisk-cuda"
    cp "$WORKSPACE/target/release/libzisk_witness.so" "$HOME/.zisk/bin/libzisk_witness_cuda.so"
    rm -rf "$WORKSPACE"

    echo "Checking for cargo-zisk-cuda CLI tool..."
    if cargo-zisk-cuda --version; then
        echo "cargo-zisk-cuda CLI tool verified successfully."
    else
        echo "Error: 'cargo-zisk-cuda --version' failed." >&2
        exit 1
    fi
fi

# Step 4: Make sure `lib-c`'s build script is ran.
#
# `ziskos` provides guest program runtime, and `lib-c` is a dependency of `ziskos`,
# when we need to compile guest, the `build.rs` of `lib-c` will need to be ran once,
# but if there are multiple `build.rs` running at the same time, it will panic.
# So here we make sure it's already ran, and the built thing will be stored in
# `$CARGO_HOME/git/checkouts/zisk-{hash}/{rev}/lib-c/c/build`, so could be
# re-used as long as the `ziskos` has the same version.
WORKSPACE="/tmp/build-lib-c"
cargo new "$WORKSPACE" --name build-lib-c
cargo add lib-c --git https://github.com/0xPolygonHermez/zisk.git --tag "v$ZISK_VERSION" --manifest-path "$WORKSPACE/Cargo.toml"
cargo build --manifest-path "$WORKSPACE/Cargo.toml"
rm -rf "$WORKSPACE"

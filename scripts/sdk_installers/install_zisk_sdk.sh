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
export ZISK_VERSION="0.9.0"
export SETUP_KEY=${SETUP_KEY:=proving}
curl "https://raw.githubusercontent.com/0xPolygonHermez/zisk/v$ZISK_VERSION/ziskup/install.sh" | sed "s/main/v$ZISK_VERSION/g" | bash
unset SETUP_KEY

# Step 2: Ensure the installed cargo-zisk binary is in PATH for this script session.
export PATH="${PATH}:${HOME}/.zisk/bin"

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

# Step 3: Build cargo-zisk-gpu from source with GPU features enabled
TEMP_DIR=$(mktemp -d)
git clone https://github.com/0xPolygonHermez/zisk.git --single-branch --branch "v$ZISK_VERSION" "$TEMP_DIR/zisk"
cd "$TEMP_DIR/zisk"
cargo build --release --features gpu
cp ./target/release/cargo-zisk "${HOME}/.zisk/bin/cargo-zisk-gpu"
cp ./target/release/libzisk_witness.so "${HOME}/.zisk/bin/libzisk_witness_gpu.so"
rm -rf "$TEMP_DIR"

echo "Checking for cargo-zisk-gpu CLI tool..."
if cargo-zisk-gpu --version; then
    echo "cargo-zisk-gpu CLI tool verified successfully."
else
    echo "Error: 'cargo-zisk-gpu --version' failed." >&2
    exit 1
fi

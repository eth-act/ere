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

# Download and run the script that installs the ziskup binary itself.
# Export USE_GPU to download pre-built cargo-zisk and zisk-worker with or without cuda support.
# Export SETUP_KEY=proving-no-consttree to download proving key without doing setup.
export ZISK_VERSION="1.2.0-alpha"
export USE_GPU=$([ -n "$CUDA" ] && echo true || echo false)
export SETUP_KEY=${SETUP_KEY:=proving-no-consttree}
curl "https://raw.githubusercontent.com/0xPolygonHermez/zisk/v$ZISK_VERSION/ziskup/ziskup" | bash
unset SETUP_KEY

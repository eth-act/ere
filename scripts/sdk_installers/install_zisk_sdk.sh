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
# LocalProver downloads the proving key at runtime when setup is needed.
export ZISK_VERSION="1.3.0-alpha"
export USE_GPU=$([ -n "$CUDA" ] && echo true || echo false)
export SETUP_KEY=none
curl "https://raw.githubusercontent.com/0xPolygonHermez/zisk/v$ZISK_VERSION/ziskup/ziskup" | bash
unset SETUP_KEY

# The ASM services build from this source. The patched one makes them exit with the prover.
# Keep the revision equal to the zisk git revision in Cargo.lock.
curl -fsSL "https://raw.githubusercontent.com/han0110/zisk/de8d48e9e24965e82732463d807efaf2a739c54d/emulator-asm/src/main.c" \
    -o "$HOME/.zisk/zisk/emulator-asm/src/main.c"

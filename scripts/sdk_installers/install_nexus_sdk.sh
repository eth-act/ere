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

echo "Installing Nexus Toolchain and SDK using Nexus (prebuilt binaries)..."

# Prerequisites for Nexus (some of these are for the SDK itself beyond Nexus)
#ensure_tool_installed "curl" "to download the Nexus installer"
#ensure_tool_installed "bash" "to run the Nexus installer"
ensure_tool_installed "rustup" "for managing Rust toolchains"
ensure_tool_installed "cargo" "as cargo-nexus is a cargo subcommand"

NEXUS_TOOLCHAIN_VERSION="nightly-2025-04-06"
NEXUS_CLI_VERSION_TAG="v0.3.4"

# Install the Nexus CLI
echo "Installing Nexus CLI from GitHub repository..."
cargo "+${NEXUS_TOOLCHAIN_VERSION}" install --git https://github.com/nexus-xyz/nexus-zkvm cargo-nexus --tag "$NEXUS_CLI_VERSION_TAG"

# Install Nexus's target
rustup "+${NEXUS_TOOLCHAIN_VERSION}" target add riscv32i-unknown-none-elf

# Verify Nexus installation
echo "Verifying Nexus CLI installation..."
if cargo-nexus --version; then
    echo "Nexus CLI installation verified successfully."
else
    echo "Error: 'cargo-nexus --version' failed. Nexus CLI might not have installed correctly." >&2
    echo "       Ensure ${HOME}/.cargo/bin is in your PATH for new shells." >&2
    exit 1
fi

echo "Verifying Nexus's target installation..."
if rustup "+${NEXUS_TOOLCHAIN_VERSION}" target list --installed | grep -q "riscv32i-unknown-none-elf"; then
    echo "Target 'riscv32i-unknown-none-elf' installation verified successfully."
else
    echo "Target 'riscv32i-unknown-none-elf' not installed correctly." >&2
    echo "       Ensure ${HOME}/.cargo/bin is in your PATH for new shells." >&2
    exit 1
fi

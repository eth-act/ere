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

# Step 1: Download and run the toolchain.

# Verify Nexus installation
echo "Verifying Nexus installation..."

echo "Checking for RISC-V target..."
if rustup target list | grep -q "riscv32i-unknown-none-elf"; then
    echo "RISC-V target 'riscv32i-unknown-none-elf' not found."
else
    echo "RISC-V 'riscv32i-unknown-none-elf' not found after installation!" >&2
    echo "Install the RISC-V target:"
    rustup target add riscv32i-unknown-none-elf
fi

echo "Checking for cargo-nexus..."
if cargo --list | grep "nexus"; then
    echo "cargo-nexus found."
else
    echo "cargo-nexus not found after installation!" >&2
    echo "Install the cargo-nexus:"
    cargo install --git https://github.com/nexus-xyz/nexus-zkvm cargo-nexus --tag 'v0.3.4'
fi

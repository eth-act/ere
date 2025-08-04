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

echo "Installing Jolt CLI..."

ensure_tool_installed "rustup" "to manage Rust toolchains (though Jolt uses default nightly)"
ensure_tool_installed "git" "to install Jolt from a git repository"
ensure_tool_installed "cargo" "to build and install Rust packages"

JOLT_REVISION="55b9830a3944dde55d33a55c42522b81dd49f87a"

# Install Jolt CLI using cargo install with +nightly
# This installs the 'jolt' binary directly to $HOME/.cargo/bin
# The ere-base image should have a compatible default nightly toolchain.
echo "Installing Jolt CLI from GitHub repository (a16z/jolt)..."
cargo +nightly install --git https://github.com/a16z/jolt --force --bins jolt --rev "$JOLT_REVISION"

# Install Jolt's toolchain
jolt install-toolchain

# Verify Jolt installation
echo "Verifying Jolt CLI installation..."
if jolt --version; then
    echo "Jolt CLI installation verified successfully."
else
    echo "Error: 'jolt --version' failed. Jolt CLI might not have installed correctly." >&2
    echo "       Ensure ${HOME}/.cargo/bin is in your PATH for new shells." >&2
    exit 1
fi

echo "Jolt CLI installation successful." 
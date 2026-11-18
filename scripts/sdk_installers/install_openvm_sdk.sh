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

echo "Installing OpenVM Toolchain..."

ensure_tool_installed "rustup" "to manage Rust toolchains"
ensure_tool_installed "git" "to install cargo-openvm from a git repository"
ensure_tool_installed "cargo" "to build and install Rust packages"

OPENVM_CLI_VERSION_TAG="v2.1.0-preview"
# `cargo-openvm` defaults to `openvm-1.94.1`, which is too old for guests that
# need Rust 1.96. Keep in sync with `DEFAULT_TOOLCHAIN` in
# `crates/compiler/openvm/src/rust_rv64ima_customized.rs`.
OPENVM_TOOLCHAIN_VERSION="openvm-1.97.1"

# Install cargo-openvm at the specified version tag
echo "Installing cargo-openvm (version ${OPENVM_CLI_VERSION_TAG}) from GitHub repository (openvm-org/openvm)..."
cargo install --locked --git https://github.com/openvm-org/openvm.git --tag "${OPENVM_CLI_VERSION_TAG}" cargo-openvm

# Verify cargo-openvm installation
echo "Verifying cargo-openvm installation..."
# The cargo-openvm is installed as `cargo-openvm`, so it's invoked as `cargo openvm`
if cargo openvm --version; then
    echo "cargo-openvm installation verified successfully."
else
    echo "Error: 'cargo openvm --version' failed. cargo-openvm might not have installed correctly." >&2
    echo "       Ensure ${HOME}/.cargo/bin is in your PATH for new shells." >&2
    exit 1
fi

# Install the customized Rust toolchain providing the built-in guest target
# `riscv64im-unknown-openvm-elf`, and link it into rustup
echo "Installing OpenVM Rust toolchain (${OPENVM_TOOLCHAIN_VERSION})..."
cargo openvm toolchain install --version "${OPENVM_TOOLCHAIN_VERSION}"

# Setup aggregation keys
cargo openvm setup

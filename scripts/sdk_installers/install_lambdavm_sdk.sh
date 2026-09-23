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

echo "Installing LambdaVM Toolchain..."

ensure_tool_installed "rustup" "to manage Rust toolchains"

# LambdaVM has no CLI, Ere links the LambdaVM crates directly. Guest programs
# are built with this nightly toolchain and `-Z build-std`, which needs
# `rust-src`, according to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/Makefile#L201
LAMBDAVM_TOOLCHAIN_VERSION="nightly-2026-02-01"

# Install the Rust toolchain LambdaVM builds guest programs with
echo "Installing LambdaVM Rust toolchain (${LAMBDAVM_TOOLCHAIN_VERSION})..."
rustup toolchain install "${LAMBDAVM_TOOLCHAIN_VERSION}" --profile minimal --component rust-src

# Verify the toolchain installation
echo "Verifying LambdaVM Rust toolchain installation..."
if rustup run "${LAMBDAVM_TOOLCHAIN_VERSION}" rustc --version; then
    echo "LambdaVM Rust toolchain installation verified successfully."
else
    echo "Error: 'rustup run ${LAMBDAVM_TOOLCHAIN_VERSION} rustc --version' failed." >&2
    exit 1
fi

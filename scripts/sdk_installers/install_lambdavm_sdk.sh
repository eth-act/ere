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

echo "Installing the LambdaVM guest toolchain..."

ensure_tool_installed "rustup" "to install the LambdaVM guest toolchain"

# LambdaVM has no CLI to install, because Ere links the LambdaVM crates directly. Guest programs
# are built with a pinned nightly toolchain and `-Z build-std`, which needs `rust-src`.
# According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/Makefile#L201
LAMBDAVM_GUEST_TOOLCHAIN="nightly-2026-02-01"

rustup toolchain install "$LAMBDAVM_GUEST_TOOLCHAIN" --profile minimal --component rust-src

# The stock compiler builds with the unpinned nightly toolchain by default.
rustup toolchain install nightly --profile minimal --component rust-src

echo "Verifying the LambdaVM guest toolchain..."
rustup run "$LAMBDAVM_GUEST_TOOLCHAIN" rustc --version

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

echo "Installing Pico Toolchain..."

ensure_tool_installed "rustup" "to manage Rust toolchains"
ensure_tool_installed "git" "to install pico-cli from a git repository"
ensure_tool_installed "cargo" "to build and install Rust packages"

# The nightly Pico builds its host crates with, and whose `rust-src` the guest
# toolchain links for `-Z build-std`, according to
# https://github.com/brevis-network/pico/blob/675303e322a24c0fc52e98b0910c4a32de8bd092/rust-toolchain
PICO_TOOLCHAIN_VERSION="nightly-2026-09-27"
# Keep in sync with the `pico-sdk` rev in the workspace `Cargo.toml`.
PICO_CLI_REV="675303e322a24c0fc52e98b0910c4a32de8bd092"

# Install the nightly toolchain Pico pins
echo "Installing Pico host Rust toolchain (${PICO_TOOLCHAIN_VERSION})..."
rustup toolchain install "${PICO_TOOLCHAIN_VERSION}" --profile minimal --component rust-src

# Install pico-cli (as `cargo-pico`) with that toolchain, since it links `pico-vm`
echo "Installing pico-cli (rev ${PICO_CLI_REV}) from GitHub repository (brevis-network/pico)..."
cargo "+${PICO_TOOLCHAIN_VERSION}" install --locked --git https://github.com/brevis-network/pico --rev "${PICO_CLI_REV}" pico-cli

# Verify pico-cli installation
echo "Verifying pico-cli installation..."
if cargo "+${PICO_TOOLCHAIN_VERSION}" pico --version; then
    echo "pico-cli installation verified successfully."
else
    echo "Error: 'cargo +${PICO_TOOLCHAIN_VERSION} pico --version' failed. pico-cli might not have installed correctly." >&2
    echo "       Ensure ${CARGO_HOME:-${HOME}/.cargo}/bin is in your PATH for new shells." >&2
    exit 1
fi

# Install the customized Rust toolchain providing the built-in guest target
# `riscv64im-pico-zkvm-elf`, and link it into rustup as `pico`
echo "Installing Pico Rust toolchain..."
cargo "+${PICO_TOOLCHAIN_VERSION}" pico install

# Verify the guest toolchain installation
echo "Verifying Pico Rust toolchain installation..."
if rustup run pico rustc --print target-list | grep -q '^riscv64im-pico-zkvm-elf$'; then
    echo "Pico Rust toolchain installation verified successfully."
else
    echo "Error: the 'pico' toolchain does not provide target 'riscv64im-pico-zkvm-elf'." >&2
    exit 1
fi

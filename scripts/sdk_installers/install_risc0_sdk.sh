#!/bin/bash
set -e

# TODO: Pull this out into its own script file
# Common utility functions for shell scripts

# Checks if a tool is installed and available in PATH.
# Usage: is_tool_installed <tool_name>
# Returns 0 if found, 1 otherwise.
is_tool_installed() {
    command -v "$1" &> /dev/null
}

# Ensures a tool is installed. Exits with an error if not.
# Usage: ensure_tool_installed <tool_name> [optional_purpose_message]
# Example: ensure_tool_installed curl "to download files"
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

echo "Installing Risc0 Toolchain using rzup (latest release versions)..."

ensure_tool_installed "curl" "to download the rzup installer"
ensure_tool_installed "bash" "as the rzup installer script uses bash"

# Install rzup itself if not already present
if ! is_tool_installed "rzup"; then
    echo "Attempting to install rzup..."
    # The rzup installer (risczero.com/install) installs rzup to $HOME/.risc0/bin
    # and should modify shell profiles like .bashrc to add it to PATH.
    curl -L https://risczero.com/install | bash

    # For the current script's execution, we need to add the rzup path explicitly
    # as the .bashrc changes won't affect this running script instance.
    RZUP_BIN_DIR="${HOME}/.risc0/bin"
    if [ -d "${RZUP_BIN_DIR}" ] && [[ ":$PATH:" != *":${RZUP_BIN_DIR}:"* ]]; then
        echo "Adding ${RZUP_BIN_DIR} to PATH for current script session."
        export PATH="${RZUP_BIN_DIR}:$PATH"
    fi

    # Re-check if rzup is now in PATH
    if ! is_tool_installed "rzup"; then
        echo "Error: rzup command not found after installation attempt." >&2
        echo "       Please check if ${RZUP_BIN_DIR} was created and if it's in your PATH for new shells." >&2
        echo "       You might need to source your ~/.bashrc or similar shell profile." >&2
        exit 1
    fi
    echo "rzup installed successfully and added to PATH for this session."
else
    echo "rzup already installed and in PATH."
fi

# Now that rzup is confirmed to be in PATH for this script, install the Risc0 toolchain
echo "Running 'rzup install' to install/update Risc0 toolchain..."

RISC0_VERSION="${RISC0_VERSION:-3.0.4}"
RISC0_CPP_VERSION="${RISC0_CPP_VERSION:-2024.1.5}"
RISC0_RUST_VERSION="${RISC0_RUST_VERSION:-1.88.0}"

rzup install cargo-risczero "$RISC0_VERSION"
rzup install cpp "$RISC0_CPP_VERSION"
rzup install r0vm "$RISC0_VERSION"
rzup install rust "$RISC0_RUST_VERSION"

# Verify Risc0 installation
echo "Verifying Risc0 installation..."
ensure_tool_installed "cargo" "as cargo-risczero needs it"
cargo risczero --version || (echo "Error: cargo risczero command failed!" >&2 && exit 1)

echo "Risc0 Toolchain installation (latest release) successful."
echo "The rzup installer might have updated your shell configuration files (e.g., ~/.bashrc, ~/.zshrc)."
echo "To ensure rzup and Risc0 tools are available in your current shell session if this was a new installation,"
echo "you may need to source your shell profile (e.g., 'source ~/.bashrc') or open a new terminal." 

# Build r0vm from source with `cuda` feature enabled
if [ -n "$CUDA" ]; then
    CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
    RISC0_BIN_DIR="$HOME/.risc0/extensions/v$RISC0_VERSION-cargo-risczero-x86_64-unknown-linux-gnu"

    TEMP_DIR=$(mktemp -d)
    git clone https://github.com/risc0/risc0.git --depth 1 --branch "v$RISC0_VERSION" "$TEMP_DIR/risc0"
    cd "$TEMP_DIR/risc0"

    # Build with feature `cuda`
    cargo build --release --features cuda --bin r0vm
    # Copy the binary to the same directory with `cargo-risczero` and `r0vm`
    cp ./target/release/r0vm "$RISC0_BIN_DIR/r0vm-cuda"
    # Create symbolic link as `cargo-risczero` and `r0vm`
    ln -s "$RISC0_BIN_DIR/r0vm-cuda" "$CARGO_HOME/bin/r0vm-cuda"

    rm -rf "$TEMP_DIR"
fi

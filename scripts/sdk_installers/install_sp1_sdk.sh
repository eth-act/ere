#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

echo "Installing Succinct SP1 Toolchain..."

# Ensure prerequisites like curl are there
if ! command -v curl &> /dev/null; then
    echo "Error: curl could not be found, please install it first." >&2
    exit 1
fi
if ! command -v bash &> /dev/null; then # sp1up script uses bash
    echo "Error: bash could not be found, please install it first." >&2
    exit 1
fi

# Define default homes if not set, useful for Docker context
DEFAULT_SP1_DIR="${HOME}/.sp1"

# Use existing ENV var or default. Docker ENV will make these available.
# For local use, user might need to add these to their .bashrc/.zshrc
export SP1_DIR="${SP1_DIR:-${DEFAULT_SP1_DIR}}"

# Run sp1up installer script
curl -L https://sp1up.succinct.xyz | bash

# Add sp1up and sp1 binaries to PATH for this script's execution context
# and for subsequent commands if this script is sourced.
export PATH="${SP1_DIR}/bin:$PATH"

export SP1_VERSION="${SP1_VERSION:-latest}"

# Run sp1up to install/update the toolchain
if ! command -v sp1up &> /dev/null; then
    echo "Error: sp1up command not found after installation script. Check PATH or installation." >&2
    exit 1
fi
sp1up -v ${SP1_VERSION} # Installs the toolchain and cargo-prove

echo "Verifying SP1 installation..."
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo command not found. Ensure Rust is installed and in PATH." >&2
    exit 1 # cargo prove needs cargo
fi

cargo prove --version
rustup toolchain list | grep succinct || (echo "Error: SP1 Toolchain (succinct) not found after install!" >&2 && exit 1)

echo "Succinct SP1 Toolchain installation successful."
echo "If running locally (not in Docker), to make SP1 commands available in your current shell or new shells, ensure the following are in your shell profile (e.g., ~/.bashrc, ~/.zshrc):"
echo "  export SP1_DIR=\"${SP1_DIR}\""
echo "  export PATH=\"${SP1_DIR}/bin:\$PATH\""
echo "Then source your profile or open a new terminal."

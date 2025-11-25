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

echo "Installing Brevis Pico Toolchain..."

ensure_tool_installed "rustup" "to manage Rust toolchains"
ensure_tool_installed "git" "to install pico-cli from a git repository"
ensure_tool_installed "cargo" "to build and install Rust packages"

PICO_TOOLCHAIN_VERSION="nightly-2025-08-04"
PICO_CLI_VERSION_TAG="v1.1.8"

# Install the specific nightly toolchain for Pico
echo "Installing Pico-specific Rust toolchain: ${PICO_TOOLCHAIN_VERSION}..."
rustup install "${PICO_TOOLCHAIN_VERSION}"
rustup component add rust-src --toolchain "${PICO_TOOLCHAIN_VERSION}"

# Install pico-cli using the specified toolchain
# cargo-pico is a cargo subcommand, typically installed to $HOME/.cargo/bin
echo "Installing pico-cli from GitHub repository (brevis-network/pico)..."
cargo "+${PICO_TOOLCHAIN_VERSION}" install --git https://github.com/brevis-network/pico pico-cli --tag "$PICO_CLI_VERSION_TAG" --locked

# Verify pico-cli installation
echo "Verifying pico-cli installation..."
# The pico-cli is installed as `cargo-pico`, so it's invoked as `cargo pico`
if cargo "+${PICO_TOOLCHAIN_VERSION}" pico --version; then
    echo "pico-cli (cargo pico) installation verified successfully."
else
    echo "Error: 'cargo pico --version' failed. pico-cli might not have installed correctly." >&2
    echo "       Ensure ${HOME}/.cargo/bin is in your PATH for new shells." >&2
    exit 1
fi

# TODO: Maybe remove this, We likely always will use `cargo pico`
echo "Brevis Pico Toolchain and pico-cli installation successful."
echo "The specified Rust toolchain (${PICO_TOOLCHAIN_VERSION}) is installed."
echo "pico-cli (as cargo-pico) is installed and should be available via 'cargo pico ...' using the above toolchain."
echo "For pico-cli to be globally available as 'cargo pico' without the +toolchain specifier,"
echo "you might need to set ${PICO_TOOLCHAIN_VERSION} as your default toolchain for the project/directory or globally if desired,"
echo "or ensure your project's rust-toolchain.toml specifies this version." 
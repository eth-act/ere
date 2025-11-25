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

ensure_tool_installed "rustup" "to manage Rust toolchains"
ensure_tool_installed "git" "to install airbender-cli from a git repository"
ensure_tool_installed "cargo" "to build and install Rust packages"

AIRBENDER_CLI_VERSION_TAG="v0.5.1"

# Install airbender-cli using the specified toolchain and version tag
echo "Installing airbender-cli (version ${AIRBENDER_CLI_VERSION_TAG}) from GitHub repository (matter-labs/zksync-airbender)..."
cargo +nightly install --locked --git https://github.com/matter-labs/zksync-airbender.git --tag "${AIRBENDER_CLI_VERSION_TAG}" ${CUDA:+-F gpu} cli

# Rename cli to airbender-cli
CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}
mv $CARGO_HOME/bin/cli $CARGO_HOME/bin/airbender-cli

# Verify airbender-cli installation
echo "Verifying airbender-cli installation..."
if airbender-cli --version; then
    echo "airbender-cli installation verified successfully."
else
    echo "Error: 'airbender-cli --version' failed. airbender-cli might not have installed correctly." >&2
    echo "       Ensure $CARGO_HOME/bin is in your PATH for new shells." >&2
    exit 1
fi

# Install cargo-binutils to objcopy ELF to binary file
rustup +nightly component add llvm-tools
cargo install cargo-binutils

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

echo "Installing ZKM Toolchain using zkmup (latest release versions)..."

# Prerequisites for zkmup
ensure_tool_installed "curl" "to download the zkmup installer"
ensure_tool_installed "sh" "as the zkmup installer script uses sh"

ZIREM_VERSION="1.2.2"

# Step 1: Download and run the script that installs the zkmup binary itself.
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/ProjectZKM/toolchain/refs/heads/main/setup.sh | sh

# Step 2: Ensure the installed zkmup script is in PATH
export PATH="${PATH}:${HOME}/.zkm-toolchain/bin"

# Step 3: Link the latest toolchain as toolchain `zkm`
rustup toolchain link zkm $(ls -d $HOME/.zkm-toolchain/* | grep "$(zkmup list-available | cut -d' ' -f1)$")
# Step 4: Install cargo-ziren by building from source
cargo +nightly install --locked --git https://github.com/ProjectZKM/Ziren.git --tag "v${ZIREM_VERSION}" zkm-cli

# Verify ZKM installation
echo "Verifying ZKM installation..."

echo "Checking for 'zkm' toolchain..."
if rustup +zkm toolchain list | grep -q "zkm"; then
    echo "ZKM Rust toolchain found."
else
    echo "Error: ZKM Rust toolchain ('zkm') not found after installation!" >&2
    exit 1
fi

echo "Checking for cargo-ziren CLI tool..."
if cargo ziren --version; then
    echo "cargo-ziren CLI tool verified successfully."
else
    echo "Error: 'cargo ziren --version' failed." >&2
    exit 1
fi
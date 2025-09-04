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

echo "Installing zkm Toolchain using zkmup (latest release versions)..."

ensure_tool_installed "curl" "to download the zkmup installer"
ensure_tool_installed "wget" "to download the zkmup installer"
ensure_tool_installed "bash" "as the zkmup installer script uses bash"

# Install zkmup itself if not already present
if ! is_tool_installed "zkmup"; then
    echo "Attempting to install zkmup..."
    # The zkmup installer (https://docs.zkm.io/introduction/installation.html) installs zkmup to $HOME/.zkm-toolchain/bin
    # and should modify shell profiles like .bashrc to add it to PATH.
    curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/ProjectZKM/toolchain/refs/heads/main/setup.sh | sh


    # For the current script's execution, we need to add the zkmup path explicitly
    # as the .bashrc changes won't affect this running script instance.
    zkmup_BIN_DIR="${HOME}/.zkm-toolchain/bin"
    if [ -d "${zkmup_BIN_DIR}" ] && [[ ":$PATH:" != *":${zkmup_BIN_DIR}:"* ]]; then
        echo "Adding ${zkmup_BIN_DIR} to PATH for current script session."
        export PATH="${zkmup_BIN_DIR}:$PATH"
    fi

    # Re-check if zkmup is now in PATH
    if ! is_tool_installed "zkmup"; then
        echo "Error: zkmup command not found after installation attempt." >&2
        echo "       Please check if ${zkmup_BIN_DIR} was created and if it's in your PATH for new shells." >&2
        echo "       You might need to source your ~/.bashrc or similar shell profile." >&2
        exit 1
    fi
    echo "zkmup installed successfully and added to PATH for this session."
else
    echo "zkmup already installed and in PATH."
fi

# Now that zkmup is confirmed to be in PATH for this script, install the zkm toolchain
echo "Running 'zkmup install' to install/update zkm toolchain..."
zkmup install

# Verify zkm installation
echo "Verifying zkm installation..."
ensure_tool_installed "cargo"
zkmup list-available || (echo "Error: zkmup list-available command failed!" >&2 && exit 1)

# Export the zkm toolchain environment variables
source ~/.zkm-toolchain/env

echo "zkm Toolchain installation (latest release) successful."
echo "The zkmup installer might have updated your shell configuration files (e.g., ~/.bashrc, ~/.zshrc)."
echo "To ensure zkmup and zkm tools are available in your current shell session if this was a new installation,"
echo "you may need to source your shell profile (e.g., 'source ~/.bashrc') or open a new terminal."
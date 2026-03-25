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

echo "Installing ZisK Toolchain and SDK using ziskup (prebuilt binaries)..."

# Prerequisites for ziskup and ZisK (some of these are for the SDK itself beyond ziskup)
ensure_tool_installed "curl" "to download the ziskup installer"
ensure_tool_installed "bash" "to run the ziskup installer"
ensure_tool_installed "rustup" "for managing Rust toolchains (ZisK installs its own)"
ensure_tool_installed "cargo" "to pre-build lib-c"

# Step 1: Download and run the script that installs the ziskup binary itself.
# Export SETUP_KEY=proving-no-consttree to download proving key without doing setup.
export ZISK_VERSION="0.16.1"
export SETUP_KEY=${SETUP_KEY:=proving-no-consttree}
curl "https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh" | bash
unset SETUP_KEY

# Step 2: Make sure `lib-c`'s build script is ran.
#
# `ziskos` provides guest program runtime, and `lib-c` is a dependency of `ziskos`,
# when we need to compile guest, the `build.rs` of `lib-c` will need to be ran once,
# but if there are multiple `build.rs` running at the same time, it will panic.
# So here we make sure it's already ran, and the built thing will be stored in
# `$CARGO_HOME/git/checkouts/zisk-{hash}/{rev}/lib-c/c/build`, so could be
# re-used as long as the `ziskos` has the same version.
WORKSPACE=$(mktemp -d)
cargo init "$WORKSPACE" --name build-lib-c
cargo add lib-c --git https://github.com/han0110/zisk.git --branch "patch/v$ZISK_VERSION" --manifest-path "$WORKSPACE/Cargo.toml"
cargo build --manifest-path "$WORKSPACE/Cargo.toml"
rm -rf "$WORKSPACE"

# FIXME: Remove this step when upgrading to `v0.17.0`
# Step 3: Rebuild `ziskemu` using the patched repo
cargo install --git https://github.com/han0110/zisk.git --branch "patch/v$ZISK_VERSION" --locked ziskemu
CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
mv $CARGO_BIN/ziskemu $HOME/.zisk/bin/ziskemu

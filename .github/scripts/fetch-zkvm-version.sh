#!/bin/bash

set -e -o pipefail

# Usage: ./fetch-zkvm-version.sh <zkvm> <crate>
# Examples:
#   .github/scripts/fetch-zkvm-version.sh airbender execution_utils
#   .github/scripts/fetch-zkvm-version.sh jolt jolt-sdk
#   .github/scripts/fetch-zkvm-version.sh miden miden-core
#   .github/scripts/fetch-zkvm-version.sh nexus nexus-sdk
#   .github/scripts/fetch-zkvm-version.sh openvm openvm-sdk
#   .github/scripts/fetch-zkvm-version.sh pico pico-vm
#   .github/scripts/fetch-zkvm-version.sh risc0 risc0-zkvm
#   .github/scripts/fetch-zkvm-version.sh sp1 sp1-sdk
#   .github/scripts/fetch-zkvm-version.sh ziren zkm-sdk
#   .github/scripts/fetch-zkvm-version.sh zisk 0xPolygonHermez/zisk

if [ $# -ne 2 ]; then
    echo "Usage: $0 <zkvm> <crate>"
    echo "  crate: crate (e.g. openvm-sdk) or github org/repo (e.g. 0xPolygonHermez/zisk)"
    exit 1
fi

ZKVM=$1
CRATE=$2

get_github_latest() {
    local org_repo=$1
    curl -sL "https://api.github.com/repos/$org_repo/tags" | grep -oP '"name":\s*"\K[^"]+' | head -1
}

get_crates_io_latest() {
    local crate=$1
    curl -sL -A "EreCI" "https://crates.io/api/v1/crates/$crate" | grep -oP '"max_version":"\K[^"]+'
}

if [[ "$CRATE" == */* ]]; then
    # It is in format of org/repo, get current version from build.rs
    LATEST=$(get_github_latest "$CRATE")
    CURRENT=$(grep -oP 'gen_name_and_sdk_version\("'"$ZKVM"'", "\K[^"]+' "crates/zkvm/$ZKVM/build.rs")
else
    # It is a crate name, get current version from Cargo.toml
    LINE=$(grep "$CRATE" Cargo.toml)

    if echo "$LINE" | grep -q "git ="; then
        # Dependency from github.com
        REPO=$(echo "$LINE" | grep -oP 'git = "https://github.com/\K[^"]+' | sed 's/\.git$//')
        CURRENT=$(echo "$LINE" | grep -oP 'tag = "\K[^"]+')
        LATEST=$(get_github_latest "$REPO")
    else
        # Dependency from crates.io
        CURRENT=$(grep "^$CRATE = " Cargo.toml | grep -oP '"\K[0-9.]+(?=")')
        LATEST=$(get_crates_io_latest "$CRATE")
    fi
fi

echo "CURRENT=v${CURRENT#v}"
echo "LATEST=v${LATEST#v}"

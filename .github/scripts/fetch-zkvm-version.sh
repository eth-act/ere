#!/bin/bash

set -e -o pipefail

# Usage: ./fetch-zkvm-version.sh <crate>
# Examples:
#   .github/scripts/fetch-zkvm-version.sh lambda-vm-prover
#   .github/scripts/fetch-zkvm-version.sh openvm
#   .github/scripts/fetch-zkvm-version.sh pico-sdk
#   .github/scripts/fetch-zkvm-version.sh sp1-sdk
#   .github/scripts/fetch-zkvm-version.sh ziskos

if [ $# -ne 1 ]; then
    echo "Usage: $0 <crate>"
    exit 1
fi

CRATE=$1

# Prints the highest semver tag without a leading v. A pre-release sorts before its release.
get_github_latest() {
    local org_repo=$1
    echo "https://github.com/$org_repo"
    git ls-remote --tags --refs "https://github.com/$org_repo" \
        | sed 's#.*refs/tags/##' \
        | grep -E '^v?[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' \
        | sed -e 's/^v//' -e 's/-/~/' | sort -V | sed 's/~/-/' | tail -1
}

get_crates_io_latest() {
    local crate=$1
    curl -sL -A "EreCI" "https://crates.io/api/v1/crates/$crate" | grep -oP '"max_version":"\K[^"]+'
}

LINE=$(grep "^$CRATE = " Cargo.toml)

if echo "$LINE" | grep -q "git ="; then
    # Dependency from github.com
    REPO=$(echo "$LINE" | grep -oP 'git = "https://github.com/\K[^"]+' | sed 's/\.git$//')
    CURRENT=$(echo "$LINE" | grep -oP 'tag = "\K[^"]+')
    LATEST=$(get_github_latest "$REPO")
else
    # Dependency from crates.io
    CURRENT=$(grep "^$CRATE = " Cargo.toml | grep -oP '"\K[0-9][^"]*')
    LATEST=$(get_crates_io_latest "$CRATE")
fi

echo "CURRENT=v${CURRENT#v}"
echo "LATEST=v${LATEST#v}"

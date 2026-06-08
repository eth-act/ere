#!/bin/bash
#
# Build the ere-verifier-c static library for one target. When an archive path
# is given, package the library together with the generated header into that
# tar.gz. Needs cargo-zigbuild and a zig toolchain. Other language bindings
# reuse this script to produce their static library.

set -euo pipefail

usage() {
    echo "usage: build.sh <target-triple> [archive-path]" >&2
    exit 1
}

[ $# -ge 1 ] || usage
TARGET="$1"
ARCHIVE="${2:-}"

WORKSPACE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="libere_verifier_c.a"

# Linux gnu targets pin a glibc 2.17 floor so the archive references widely
# available symbols. Other targets build as given.
case "$TARGET" in
    *-unknown-linux-gnu) ZIGBUILD_TARGET="$TARGET.2.17" ;;
    *)                   ZIGBUILD_TARGET="$TARGET" ;;
esac

rustup target add "$TARGET" >/dev/null 2>&1 || true
cargo zigbuild --release --manifest-path "$WORKSPACE/Cargo.toml" --target "$ZIGBUILD_TARGET" -p ere-verifier-c

if [ -n "$ARCHIVE" ]; then
    mkdir -p "$(dirname "$ARCHIVE")"
    tar -czf "$ARCHIVE" \
        -C "$WORKSPACE/target/$TARGET/release" "$LIB" \
        -C "$WORKSPACE/bindings/c/build" ere_verifier.h
fi

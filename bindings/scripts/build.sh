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

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="libere_verifier_c.a"

WORKSPACE="$(mktemp -d)"
trap 'rm -rf "$WORKSPACE"' EXIT

# The generated header names the API to keep.
api_symbols() {
    grep -oE '\bere_[a-z0-9_]+\(' "$REPO/bindings/c/build/ere_verifier.h" | tr -d '(' | sort -u
}

# Global symbols as plain C names.
exported_symbols() {
    "$1" --defined-only --extern-only "$2" \
        | awk '$1 ~ /^[0-9a-f]+$/ && $NF !~ /^DW\.ref\./ { sub(/^_/, "", $NF); print $NF }' \
        | sort -u
}

assert_exports_match_api() {
    local api
    api="$(api_symbols)"
    if [ "$1" != "$api" ]; then
        echo "$LIB does not export exactly the API of ere_verifier.h" >&2
        diff <(echo "$api") <(echo "$1") >&2 || true
        exit 1
    fi
}

elf_tool() {
    local name="$1"
    local prefix="${TARGET%%-*}-linux-gnu-"

    if command -v "$prefix$name" > /dev/null; then
        echo "$prefix$name"
    elif [ "$(uname -m)" = "${TARGET%%-*}" ]; then
        echo "$name"
    else
        echo "build.sh needs $prefix$name to localize the symbols of $TARGET" >&2
        exit 1
    fi
}

# The bundled Rust runtime collides with the runtime of a Rust consumer. Only the
# `ere_` API stays global, plus the weak anchor every `.eh_frame` entry needs.
localize_elf_symbols() {
    local library="$1"
    local ld objcopy ar nm
    ld="$(elf_tool ld)"
    objcopy="$(elf_tool objcopy)"
    ar="$(elf_tool ar)"
    nm="$(elf_tool nm)"

    "$ld" -r --whole-archive "$library" --no-whole-archive -o "$WORKSPACE/merged.o"
    # The archiver cannot read the merged bitcode.
    "$objcopy" --wildcard \
        --keep-global-symbol='ere_*' \
        --keep-global-symbol='DW.ref.rust_eh_personality' \
        --remove-section=.llvmbc \
        --remove-section=.llvmcmd \
        "$WORKSPACE/merged.o" "$WORKSPACE/localized.o"
    "$ar" crs "$WORKSPACE/localized.a" "$WORKSPACE/localized.o"
    mv "$WORKSPACE/localized.a" "$library"

    assert_exports_match_api "$(exported_symbols "$nm" "$library")"
}

localize_macho_symbols() {
    local library="$1"
    local nm
    # The LLVM of Xcode is too old to read the bitcode that rust embeds.
    nm="$(find "$(rustc --print sysroot)" -name llvm-nm -type f | head -1)"
    if [ -z "$nm" ]; then
        echo "build.sh needs the llvm-tools component of the rust toolchain" >&2
        exit 1
    fi

    api_symbols | sed 's/^/_/' > "$WORKSPACE/exported.txt"
    # A partial link loads almost nothing of an archive, so pass the members.
    mkdir -p "$WORKSPACE/objects"
    (cd "$WORKSPACE/objects" && ar x "$library")
    ld -arch arm64 -platform_version macos 11.0 "$(xcrun --show-sdk-version)" \
        -r "$WORKSPACE"/objects/*.o \
        -exported_symbols_list "$WORKSPACE/exported.txt" -o "$WORKSPACE/merged.o"
    libtool -static -o "$WORKSPACE/localized.a" "$WORKSPACE/merged.o"
    mv "$WORKSPACE/localized.a" "$library"

    assert_exports_match_api "$(exported_symbols "$nm" "$library")"
}

# Linux gnu targets pin a glibc 2.17 floor so the archive references widely
# available symbols. Other targets build as given.
case "$TARGET" in
    *-unknown-linux-gnu) ZIGBUILD_TARGET="$TARGET.2.17" ;;
    *)                   ZIGBUILD_TARGET="$TARGET" ;;
esac

rustup target add "$TARGET" >/dev/null 2>&1 || true
cargo zigbuild --release --manifest-path "$REPO/Cargo.toml" --target "$ZIGBUILD_TARGET" -p ere-verifier-c

# The work happens on a copy, so a failed run leaves the build output untouched
# rather than feeding a half localized archive to the next one.
LIBRARY="$REPO/target/$TARGET/release/$LIB"
cp "$LIBRARY" "$WORKSPACE/$LIB"
case "$TARGET" in
    aarch64-apple-darwin) localize_macho_symbols "$WORKSPACE/$LIB" ;;
    *-apple-darwin)       echo "build.sh localizes arm64 macOS only, not $TARGET" >&2; exit 1 ;;
    *)                    localize_elf_symbols "$WORKSPACE/$LIB" ;;
esac
cp "$WORKSPACE/$LIB" "$LIBRARY"

if [ -n "$ARCHIVE" ]; then
    mkdir -p "$(dirname "$ARCHIVE")"
    tar -czf "$ARCHIVE" \
        -C "$REPO/target/$TARGET/release" "$LIB" \
        -C "$REPO/bindings/c/build" ere_verifier.h
fi

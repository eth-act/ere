#!/usr/bin/env bash
# Turns a Rust staticlib built with `-Clinker-plugin-lto` into a vendor archive that exports only
# the zkVM guest ABI listed in `abi.txt`.
#
# All bitcode members are merged into one module and every other symbol is internalized, so the
# vendor's copy of `core`, its panic handler and its allocator cannot collide with the guest's.
# That module is then compiled into one fat LTO object: native RV64IM code, with the zkVM's ISA
# extensions, for any linker, plus the same module as LLVM bitcode in its `.llvm.lto` section, so a
# guest link with `ld.lld --fat-lto-objects` can inline across the `zkvm_*` boundary.
#
# Compiler runtime builtins (`compiler_builtins`) are left out: the guest's toolchain supplies them,
# so a link holds one copy of each. Other native members are kept as they are.
#
# Usage: vendor-archive.sh <input.a> <output.a> [target-features]
#   target-features: the zkVM's ISA extensions beyond RV64IM (`zkvm.features`), e.g. `+zba,+zbb`.
set -euo pipefail

LLVM_BIN=${LLVM_BIN:-/opt/homebrew/opt/llvm@22/bin}
here=$(cd "$(dirname "$0")" && pwd)
input=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
output=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
features=${3:-}
exports=$here/abi.txt

work=$(mktemp -d "${AGENT_TMPDIR:-${TMPDIR:-/tmp}}/vendor-archive.XXXXXX")
trap 'rm -rf "$work"' EXIT
cd "$work"
"$LLVM_BIN/llvm-ar" x "$input"

bitcode=() native=()
for member in *; do
    if [[ $(head -c 4 "$member" | xxd -p) == 4243c0de ]]; then
        bitcode+=("$member")
    elif [[ $member != compiler_builtins-* ]]; then
        native+=("$member")
    fi
done

# A native member is vendor code rustc kept out of LTO, for example OpenVM's `#![no_builtins]`
# memcpy. The guest's weak `compiler_builtins` definitions would stop the linker from ever
# extracting it, so give it an anchor symbol that `zkvm.ld` names in `EXTERN`, which links it
# unconditionally and lets its strong definitions win.
anchor=0 archive=$(basename "$output" .a)
for member in ${native[@]+"${native[@]}"}; do
    "$LLVM_BIN/llvm-objcopy" --add-symbol "__zkvm_staticlib_keep_${archive}_$anchor=0,global" "$member"
    anchor=$((anchor + 1))
done

"$LLVM_BIN/llvm-link" "${bitcode[@]}" -o merged.bc

# Module-level asm (for example a `_start` in `global_asm!`) can name a Rust symbol the IR does not
# see as used. Keep every symbol the module both defines and references, or it would be removed.
"$LLVM_BIN/llvm-nm" merged.bc | awk '$1 == "U" {print $2}' | sort -u > undefined.txt
"$LLVM_BIN/llvm-nm" --defined-only merged.bc | awk '{print $NF}' | sort -u > defined.txt
public=$( (cat "$exports"; comm -12 undefined.txt defined.txt) | sort -u | paste -sd, -)

# Vendors mark some functions `#[inline(never)]`, a choice made for their own non-LTO builds. At
# this boundary the guest link's LTO decides, as it would inside a vendor's own guest, so drop
# `noinline` from every ABI export.
uninline=()
while read -r symbol; do uninline+=("-force-remove-attribute=$symbol:noinline"); done < "$exports"

"$LLVM_BIN/opt" merged.bc -o vendor.bc -passes='forceattrs,internalize,globaldce' \
    "${uninline[@]}" -internalize-public-api-list="$public"

# The fat LTO object. The code model must be given: code generation from bitcode does not take it
# from the module, and the default (`medlow`) cannot reach memory above 2 GiB, where ZisK's RAM is.
target_features=()
for feature in ${features//,/ }; do target_features+=(-Xclang -target-feature -Xclang "$feature"); done
"$LLVM_BIN/clang" --target=riscv64-unknown-elf -march=rv64im -mabi=lp64 -mcmodel=medany \
    ${target_features[@]+"${target_features[@]}"} -Wno-override-module \
    -O3 -flto=full -ffat-lto-objects -c vendor.bc -o vendor.o

rm -f "$output"
"$LLVM_BIN/llvm-ar" rcs "$output" vendor.o ${native[@]+"${native[@]}"}

# An ABI function the vendor does not define is not an error here: a guest that calls it fails to
# link, and one that does not is unaffected.
"$LLVM_BIN/llvm-nm" --defined-only --extern-only "$output" | awk 'NF == 3 {print $3}' | sort -u > exported.txt
sort -u "$exports" | comm -23 - exported.txt | while read -r symbol; do
    echo "warning: $output does not define $symbol (abi.txt)" >&2
done

echo "$output: 1 fat LTO object, ${#native[@]} other native members"

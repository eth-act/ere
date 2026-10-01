#!/usr/bin/env bash
# Builds one zkVM static library: a directory holding `libzkvm.a` (the vendor archive, one fat LTO
# object exporting only the guest ABI in `abi.txt`), `zkvm.ld` (the vendor linker script, which
# pulls the archive in with `INPUT(-lzkvm)`) and, if the zkVM supports more than RV64IM,
# `zkvm.features` (the LLVM target features a guest link adds to the guest's code) and optionally
# `zkvm-lto-plugin.so` (an LLVM pass plugin a guest link runs at the end of LTO).
#
# Until zkVM teams publish static libraries, ere builds them from its platform crates, with stock
# nightly Rust and `-Clinker-plugin-lto`: `ere-platform-<zkvm>`, built as a staticlib for the target
# spec in its `staticlib/` directory (the generic target with only `os`/`vendor` changed, so the
# vendor's cfgs select its accelerated paths), next to its linker script (`zkvm.ld`) and, for ZisK,
# its LTO plugin (`lto-plugin/`). The workspace's patches apply.
#
# Usage: build.sh <openvm|zisk|sp1> <out-dir>
# Requires: the pinned nightly (RUST_TOOLCHAIN) with rust-src, and LLVM tools no older than its
# LLVM (LLVM_BIN).
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
zkvm=$1
mkdir -p "$2"
out=$(cd "$2" && pwd)
LLVM_BIN=${LLVM_BIN:-/opt/homebrew/opt/llvm@22/bin}
# The pinned nightly (LLVM 22.1.0). Guest objects must come from an LLVM no newer than the linker's,
# so the static library and the guest build use the same one.
RUST_TOOLCHAIN=${RUST_TOOLCHAIN:-nightly-2026-03-17}
export LLVM_BIN CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$here/target}"

# `features`: LLVM target features the zkVM supports beyond RV64IM, which a guest link adds to the
# guest's code (`zkvm.features`). They match what each vendor's own guest builds use:
# OpenVM's target enables misaligned scalar access; ZisK's carries Zba/Zbb/Zbkb/Zbs and proves
# misaligned access more cheaply than the byte loads it replaces. SP1's executor rejects misaligned
# loads and has no bitmanip, so it gets none.
# `crate_features`: the platform crate's features. The vendor's heap is the `embedded-alloc`
# stand-in the workspace patches in: OpenVM's with `heap-embedded-alloc`, ZisK's with
# `zisk-embedded-tlfs-alloc` (and without `user-hints`, which pulls in tokio for the host prover),
# SP1's always. ZisK's `panic-handler` gives the staticlib the handler a guest program would.
case $zkvm in
    openvm) spec=riscv64im-openvm-elf std=core,alloc,panic_abort features=+unaligned-scalar-mem
        crate_features=(--features heap-embedded-alloc) ;;
    zisk) spec=riscv64im-zisk-zkvm-elf std=core,alloc,panic_abort
        features=+zba,+zbb,+zbkb,+zbs,+unaligned-scalar-mem
        crate_features=(--no-default-features
            --features inputcpy,zisk-embedded-tlfs-alloc,panic-handler) ;;
    sp1) spec=riscv64im-succinct-zkvm-elf std=std,panic_abort features=
        crate_features=(--no-default-features) ;;
    *) echo "unknown zkVM: $zkvm" >&2; exit 1 ;;
esac
dir=$root/crates/platform/$zkvm/staticlib

# The vendor's own code gets the same features as the guest's, as in the vendor's own builds. The
# release profile is the staticlib's: one codegen unit, LTO left to the guest link, and aborting
# panics.
(cd "$root" && RUSTFLAGS="-Clinker-plugin-lto -Cpasses=lower-atomic --cfg getrandom_backend=\"custom\"${features:+ -Ctarget-feature=$features}" \
    cargo "+$RUST_TOOLCHAIN" rustc --release --locked -p "ere-platform-$zkvm" "${crate_features[@]}" \
    --crate-type staticlib --config 'profile.release.codegen-units=1' \
    --config 'profile.release.lto="off"' --config 'profile.release.panic="abort"' \
    -Zbuild-std="$std" -Zbuild-std-features=compiler-builtins-mem -Zjson-target-spec \
    --target "$dir/$spec.json")
input='INPUT(-lzkvm)'
"$here/vendor-archive.sh" "$CARGO_TARGET_DIR/$spec/release/libere_platform_$zkvm.a" "$out/libzkvm.a" "$features"

{
    printf '/* Pulls in the static library: a guest link needs only -T this script and -L its directory. */\n'
    printf '%s\n' "$input"
    # Native vendor members that must always be linked, see `vendor-archive.sh`.
    keep=$("$LLVM_BIN/llvm-nm" "$out/libzkvm.a" 2>/dev/null |
        awk '$NF ~ /^__zkvm_staticlib_keep_/ {print $NF}' | sort -u | paste -sd' ' -)
    [[ -n $keep ]] && printf 'EXTERN(%s)\n' "$keep"
    printf '\n'
    cat "$dir/zkvm.ld"
} >"$out/zkvm.ld"
if [[ -n $features ]]; then echo "$features" >"$out/zkvm.features"; else rm -f "$out/zkvm.features"; fi

# The zkVM's LLVM pass plugin (`staticlib/lto-plugin/`), if it has one, run by a guest link at the end
# of LTO. It is native code for this machine and must be built against the LLVM of the `ld.lld`
# that loads it.
if [[ -d $dir/lto-plugin ]]; then
    cmake -S "$dir/lto-plugin" -B "$CARGO_TARGET_DIR/lto-plugin-$zkvm" -DCMAKE_BUILD_TYPE=Release \
        -DLLVM_DIR="$LLVM_BIN/../lib/cmake/llvm" -DCMAKE_C_COMPILER="$LLVM_BIN/clang" \
        -DCMAKE_CXX_COMPILER="$LLVM_BIN/clang++" >/dev/null
    cmake --build "$CARGO_TARGET_DIR/lto-plugin-$zkvm" >/dev/null
    cp "$CARGO_TARGET_DIR/lto-plugin-$zkvm/zkvm-lto-plugin.so" "$out/"
else
    rm -f "$out/zkvm-lto-plugin.so"
fi
echo "$out"

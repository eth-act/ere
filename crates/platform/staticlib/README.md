# zkVM static libraries

`build.sh` builds a zkVM's platform crate, `ere-platform-<zkvm>`, as a static library that a guest
naming no zkVM links against to run on that zkVM: the zkvm-standards "Static Library and Linker
Script", plus two optional files.

```text
<out>/
├── libzkvm.a           one fat LTO object exporting exactly the guest ABI (`abi.txt`)
├── zkvm.ld             the linker script; `INPUT(-lzkvm)` pulls the archive into the link
├── zkvm.features       optional: ISA extensions beyond RV64IM, for the guest's code at link time
└── zkvm-lto-plugin.so  optional: LLVM pass plugin for the end of LTO (native, per host)
```

`libzkvm.a` holds native RV64IM code for any linker and the same code as LLVM bitcode, which
`ld.lld --fat-lto-objects` optimizes with the guest's, so accelerator calls inline. It carries no
compiler runtime builtins; the guest's toolchain supplies them.

## ABI

`abi.txt`: `_start`, the I/O interface (`read_input`, `write_output`, `abort`), the `zkvm_*`
accelerators and the optional memory functions (`memcpy`, `memmove`, `memset`, `memcmp`).
`vendor-archive.sh` internalizes everything else, so the vendor's `core`, panic handler and
allocator cannot collide with the guest's, and warns about any ABI symbol a library lacks.

## Memory

```text
_end ── vendor region ── _heap_start ── guest heap ── _heap_end
```

The guest's heap is `_heap_start` to `_heap_end`, with the guest's own allocator. The vendor's code
allocates from its own region, `_end` to `_heap_start` (256 MiB for SP1, 128 MiB for OpenVM, 64 MiB
for ZisK, set in `zkvm.ld`), through its runtime's `embedded-alloc` heap, which the workspace's
`[patch]` replaces with `embedded-alloc/`: a stack that frees in any order. Freeing the newest block
pops it and every freed block beneath it, so each accelerator call leaves the region as it found
it.

## Build

```bash
crates/platform/staticlib/build.sh <openvm|sp1|zisk> <out-dir>
```

Requires nightly Rust with `rust-src` and LLVM tools no older than its LLVM (`LLVM_BIN`, default
Homebrew `llvm@22`). `build.sh` runs `cargo rustc --crate-type staticlib` for the target spec in
`crates/platform/<zkvm>/staticlib/` (the generic target with the zkVM's `os`/`vendor`, so the
vendor's cfgs select its accelerated paths), then `vendor-archive.sh`, then adds the linker script,
features and plugin from the same directory.

Pushing a tag `staticlib-*` runs `.github/workflows/release-staticlib.yml`, which publishes
`staticlib-<zkvm>.tar.gz` and `SHA256SUMS`; the ZisK plugin there is built for Linux x86_64 and
LLVM 22.

## Vendor notes

- SP1: `libzkevm` v6.6.0 does not follow the standard encoding for `zkvm_bls12_*` and
  `zkvm_ripemd160`; the workspace pins the fix ere's SP1 test guests use
  (succinctlabs/sp1#2865). SP1 builds guests with
  `-misched-prera-direction=bottomup -misched-postra-direction=bottomup`, which a guest link
  passes to LTO. No `zkvm.features`: SP1 rejects misaligned loads.
- OpenVM: `+unaligned-scalar-mem`, which `openvm-mem` needs. `zkvm.ld` follows
  `openvm-platform`'s layout and keeps the ELF headers out of the stack.
- ZisK: `+zba,+zbb,+zbkb,+zbs,+unaligned-scalar-mem`. The LTO plugin lowers memory operations
  longer than 16 bytes, or of runtime length, to ZisK's inline DMA precompiles, as ZisK's own
  compiler does; without it, BN254 pairing and KZG blocks take about three times as many steps.
  It must be built against the LLVM of the `ld.lld` that loads it. `zkvm.ld` puts `.data` in the
  `.bss` segment, since ZisK rejects an empty segment at address 0.
- Vendor assembly keeps a few symbols outside the ABI exported: `__start` (OpenVM, SP1) and
  `ZISK_BUMP_HEAP_POS`/`ZISK_BUMP_HEAP_TOP` (ZisK).

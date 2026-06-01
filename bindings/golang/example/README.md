# Go binding example

This example verifies a zkVM proof through the ere Go binding.

## Obtain the static library

A consumer downloads the prebuilt archive for their platform from the releases
page and extracts it into a directory they control.

```bash
curl -fsSL https://github.com/eth-act/ere/releases/latest/download/libere_verifier_c.linux-amd64.tar.gz | tar xz
```

The archive contains libere_verifier_c.a and ere_verifier.h.

## Run against the repository fixtures

For a quick local trial, build the library from source. The build also
generates the header under bindings/c/build through the C crate build script.
The build produces a static and a shared library side by side, so copy the
static library and the header into one directory, then point cgo at it for the
header and the linker at it for the library. The fixtures under crates/verifier
supply a valid airbender proof.

```bash
cargo build --release -p ere-verifier-c
WORKSPACE="$(dirname $(cargo locate-project --message-format plain))"

mkdir -p /tmp/ere-lib
cp "$WORKSPACE/target/release/libere_verifier_c.a" /tmp/ere-lib/
cp "$WORKSPACE/bindings/c/build/ere_verifier.h" /tmp/ere-lib/

CGO_CFLAGS="-I/tmp/ere-lib" \
  go run -ldflags="-extldflags '-L/tmp/ere-lib'" . \
  -kind airbender \
  -vk "$WORKSPACE/crates/verifier/airbender/tests/fixtures/program_vk.bin" \
  -proof "$WORKSPACE/crates/verifier/airbender/tests/fixtures/proof.bin" \
  -pub "$WORKSPACE/crates/verifier/airbender/tests/fixtures/public_values.bin"
```

A successful run prints the verified public values.

## Use from your own module

After running go get on the binding module, download and extract the release
archive for your platform. It holds the header and the static library. Point
cgo at that directory for the header and the linker at it for the library.

```bash
CGO_CFLAGS="-I/path/to/extracted" \
  go run -ldflags="-extldflags '-L/path/to/extracted'" .
```

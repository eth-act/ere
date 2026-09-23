# Conformance dashboard

A static site that shows ACT4 RISC-V architectural test results for each zkVM.
Every result comes from the `ere-server-{zkvm}` images that ere CI publishes to
`ghcr.io/eth-act/ere`. Serve this directory with any static file server, for
example `python3 -m http.server -d dashboard`.

- `index.html`, `zkvm.html`: the site; there is no build step.
- `config.json`: zkVM metadata and the ACT4 version in use.
- `data/history/{zkvm}-{suite}.json`: one entry per run, appended by
  `ere-conformance run`. Each entry records the ere revision, the image digest
  and the zkVM SDK version that were tested.

## Reproduce a run

The test ELFs come from a [zkevm-test-monitor] checkout (`./run elfs`), which
needs Docker and `jq`.

```bash
git clone https://github.com/eth-act/zkevm-test-monitor ../zkevm-test-monitor
cargo run --release -p ere-conformance -- run \
    --rev v0.18.1 --zkvm zisk --mode verify \
    --monitor-path ../zkevm-test-monitor
```

- `--mode execute` needs only Docker. `prove` and `verify` need an NVIDIA GPU.
- Each test runs in a fresh `ere-server-{zkvm}` container, so containers of one
  zkVM cannot run concurrently.
- Set `ERE_DOCKER_MEMORY` (for example `64g`) to cap the containers' memory.

## Pass criteria

SP1 and OpenVM report a failing ACT4 test (non-zero exit code) as an error.
ZisK ignores the exit code, so the monitor's ZisK halt macros also write `PASS`
or `FAIL` to public output 0, and a ZisK test passes only when it reports
`PASS`. In `prove` and `verify` modes the same check applies to the public
values of the proof.

[zkevm-test-monitor]: https://github.com/eth-act/zkevm-test-monitor

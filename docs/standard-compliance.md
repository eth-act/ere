# Standards Compliance

Ere adds wrappers on top of zkVM SDKs so that guests and verifiers meet the [zkevm-standards](https://github.com/eth-act/zkevm-standards). This page lists those wrappers for each zkVM. It is based on the main branch of the standards and the SDK versions in [Supported zkVMs](../README.md#supported-zkvms).

## LambdaVM

Ere adds no compliance wrapper for LambdaVM.

## OpenVM

- [`ere-platform-openvm`](../crates/platform/openvm) exports the `read_input` and `write_output` functions of the [IO Interface](https://github.com/eth-act/zkevm-standards/blob/main/standards/io-interface/README.md) with `openvm::io::read_vec` and `openvm::io::reveal_u64`, because OpenVM exports neither function. The output is limited to 256 bytes.

## Pico

Ere adds no compliance wrapper for Pico.

## SP1

- [`ere-verifier-sp1`](../crates/verifier/sp1) rejects a proof with a non-zero exit code, so that it is a Type 1 verifier as the [Execution Termination Semantics](https://github.com/eth-act/zkevm-standards/blob/main/standards/standard-termination-semantics/README.md) define.

## ZisK

Ere adds no compliance wrapper for ZisK.

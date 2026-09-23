use ere_platform_core::Platform;
// Links the [zkvm-standards] FFI symbols `read_input` and `write_output`, and the `_start`
// entry point that calls the guest's `main`.
use lambda_vm_syscalls as _;

/// LambdaVM [`Platform`] implementation.
///
/// `read_input` and `write_output` are inherited from the trait's default
/// implementation, which calls [zkvm-standards] FFI symbols exported by `lambda-vm-syscalls`.
///
/// `print` is inherited as a no-op, because LambdaVM's `Print` ecall has no receiver on the
/// ecall bus, and a proof of a program that uses it fails verification.
///
/// `cycle_count` is inherited as `0`, because LambdaVM has no guest cycle counter.
///
/// Note that LambdaVM enforces a 1 MiB output cap at the runtime level.
///
/// LambdaVM guests are `std` programs with a plain `fn main()`, without `#![no_main]` or an
/// `entrypoint!` macro. The `_start` entry point in `lambda-vm-syscalls` calls `main`.
///
/// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
pub struct LambdaVMPlatform;

impl Platform for LambdaVMPlatform {}

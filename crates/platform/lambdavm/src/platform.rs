use ere_platform_core::Platform;

/// LambdaVM [`Platform`] implementation.
///
/// `read_input` and `write_output` are inherited from the trait's default
/// implementation, which calls [zkvm-standards] FFI symbols exported by
/// `lambda-vm-syscalls`.
///
/// `print` is inherited as a no-op, because LambdaVM's print ecall has no
/// receiver, so a proof of a program that prints fails verification.
///
/// Note that LambdaVM enforces a 1 MiB output cap at the runtime level, and
/// guest programs are `std` programs with a plain `fn main()`, which the
/// `_start` of `lambda-vm-syscalls` calls.
///
/// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
pub struct LambdaVMPlatform;

impl Platform for LambdaVMPlatform {}

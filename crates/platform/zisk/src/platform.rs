#![allow(unexpected_cfgs)]

use ere_platform_core::Platform;

/// ZisK [`Platform`] implementation.
///
/// `read_input` and `write_output` are inherited from the trait's default
/// implementation, which calls [zkvm-standards] FFI symbols exported by `ziskos`.
///
/// Note that ZisK enforces a 256-byte output cap at the runtime level.
///
/// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
pub struct ZiskPlatform;

impl Platform for ZiskPlatform {
    fn print(message: &str) {
        unsafe { sys_write(1, message.as_ptr(), message.len()) };
    }

    fn cycle_scope_start(_name: &str) {
        // NOTE: If the profile syscall is emitted, the ELF can NOT be proved by ASM prover.
        #[cfg(all(
            feature = "cycle-scope",
            all(target_os = "zkvm", target_vendor = "zisk")
        ))]
        {
            use ziskos::{
                PROFILE_REPORT_START_COST_ID, PROFILE_REPORT_START_STEPS_ID, SYSCALL_PROFILE_ID,
            };
            let name = &_name as *const &str as usize;
            ziskos::ziskos_syscall!(SYSCALL_PROFILE_ID, PROFILE_REPORT_START_COST_ID, name);
            ziskos::ziskos_syscall!(SYSCALL_PROFILE_ID, PROFILE_REPORT_START_STEPS_ID, name);
        }
    }

    fn cycle_scope_end(_name: &str) {
        // NOTE: If the profile syscall is emitted, the ELF can NOT be proved by ASM prover.
        #[cfg(all(
            feature = "cycle-scope",
            all(target_os = "zkvm", target_vendor = "zisk")
        ))]
        {
            use ziskos::{
                PROFILE_REPORT_END_COST_ID, PROFILE_REPORT_END_STEPS_ID, SYSCALL_PROFILE_ID,
            };
            let name = &_name as *const &str as usize;
            ziskos::ziskos_syscall!(SYSCALL_PROFILE_ID, PROFILE_REPORT_END_STEPS_ID, name);
            ziskos::ziskos_syscall!(SYSCALL_PROFILE_ID, PROFILE_REPORT_END_COST_ID, name);
        }
    }
}

unsafe extern "C" {
    /// POSIX-style `write` syscall exported by `ziskos`.
    fn sys_write(fd: u32, write_ptr: *const u8, nbytes: usize);
}

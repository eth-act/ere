//! ZisK execution instance.

use std::time::Duration;

use ere_prover_core::PublicValues;
use zisk_core::ZiskRom;
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
use zisk_core::zisk_ops::ZiskOp;

use crate::error::Error;

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
mod asm;
mod emu;

/// Runs a program on the ASM emulator on x86_64 Linux, and on the Rust emulator on other targets
/// and for programs with the profile operations of the `cycle-scope` guest feature, which the ASM
/// generator does not support.
pub(crate) struct ZiskExecutor {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    asm: Option<asm::AsmExecutor>,
}

impl ZiskExecutor {
    #[cfg_attr(
        not(all(target_arch = "x86_64", target_os = "linux")),
        allow(unused_variables)
    )]
    pub(crate) fn new(elf: &[u8], rom: &ZiskRom) -> Self {
        Self {
            #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
            asm: rom
                .insts
                .values()
                .all(|inst| inst.i.op != ZiskOp::Profile.code())
                .then(|| asm::AsmExecutor::new(elf)),
        }
    }

    /// Runs the framed `stdin` on `rom`.
    pub(crate) fn execute(
        &self,
        rom: &ZiskRom,
        stdin: Vec<u8>,
    ) -> Result<(PublicValues, Duration), Error> {
        #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
        if let Some(asm) = &self.asm {
            return asm.execute(rom, &stdin);
        }
        emu::execute(rom, stdin)
    }
}

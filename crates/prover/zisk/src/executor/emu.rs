//! Execution on the Rust emulator.

use std::{
    panic::{self, AssertUnwindSafe},
    time::{Duration, Instant},
};

use ere_prover_core::PublicValues;
use zisk_core::ZiskRom;
use ziskemu::{Emu, EmuOptions};

use crate::{error::Error, sdk::panic_msg};

/// Runs the framed `stdin` on `rom`.
pub(crate) fn execute(rom: &ZiskRom, stdin: Vec<u8>) -> Result<(PublicValues, Duration), Error> {
    let mut emu = Emu::new(rom);

    let start = Instant::now();
    panic::catch_unwind(AssertUnwindSafe(|| {
        emu.ctx = emu.create_emu_context(stdin, &EmuOptions::default());
        emu.run_fast(&EmuOptions::default());
    }))
    .map_err(|err| Error::EmulatorPanic(panic_msg(err)))?;
    let execution_duration = start.elapsed();

    if !emu.ctx.inst_ctx.end {
        return Err(Error::EmulatorNotTerminated);
    }

    if emu.ctx.inst_ctx.error {
        return Err(Error::EmulatorError);
    }

    Ok((emu.get_output_8().into(), execution_duration))
}

//! ZisK cost estimation.
//!
//! ZisK prices an execution as a weighted sum over the work the prover has to do, which the
//! emulator accumulates while it runs. Enabling `EmuOptions::stats` is what the `ziskemu -X` flag
//! sets, and the resulting distribution is read back from the emulator in process rather than by
//! shelling out. The emulator also prints that report to stdout, which it does for every run with
//! stats enabled.

use std::{
    collections::BTreeMap,
    panic::{self, AssertUnwindSafe},
};

use zisk_common::EmuTrace;
use zisk_core::ZiskRom;
use ziskemu::{Emu, EmuOptions};

use crate::{error::Error, sdk::panic_msg};

/// Cost kind holding the whole.
pub const TOTAL: &str = "total";

/// Cost kind covering the ROM and the lookup tables.
pub const BASE: &str = "base";

/// Cost kind covering the Main AIR.
pub const MAIN: &str = "main";

/// Cost kind covering memory.
pub const MEMORY: &str = "memory";

/// Cost kind covering the ZisK instructions the program runs.
pub const OPCODES: &str = "opcodes";

/// Cost kind covering the precompiles the program calls.
pub const PRECOMPILES: &str = "precompiles";

/// Cost kinds that partition a ZisK total, in stack order.
///
/// These are also the only rows read out of the emulator's report. `VARIABLE` is skipped because it
/// is `TOTAL` less `BASE`, `FROPS` because it re-counts opcodes already priced under `OPCODES`, and
/// `STEPS` because it counts work rather than pricing it.
const KINDS: [&str; 5] = [BASE, MAIN, MEMORY, OPCODES, PRECOMPILES];

/// The kinds worth reading out of the report, which are [`KINDS`] and the total they sum to.
fn kinds() -> impl Iterator<Item = &'static str> {
    KINDS.into_iter().chain([TOTAL])
}

/// Runs `stdin` on `rom` with the emulator's statistics enabled.
pub(crate) fn estimate_cost(rom: &ZiskRom, stdin: Vec<u8>) -> Result<BTreeMap<String, u64>, Error> {
    let options = EmuOptions {
        stats: true,
        ..Default::default()
    };
    let mut emu = Emu::new(rom);

    panic::catch_unwind(AssertUnwindSafe(|| {
        emu.run(stdin, &options, None::<Box<dyn Fn(EmuTrace)>>);
    }))
    .map_err(|err| Error::EmulatorPanic(panic_msg(err)))?;

    if !emu.terminated() {
        return Err(Error::EmulatorNotTerminated);
    }

    if emu.ctx.inst_ctx.error {
        return Err(Error::EmulatorError);
    }

    emu.ctx.stats.set_use_thousands_sep(false);
    parse(&emu.ctx.stats.report(rom))
}

/// Reads the cost distribution out of the emulator's report.
///
/// Each kind is taken from its first occurrence, which is the summary block at the top; the
/// sections below it open with headers that reuse the same labels.
fn parse(report: &str) -> Result<BTreeMap<String, u64>, Error> {
    let mut cost = BTreeMap::new();
    for line in report.lines() {
        let mut fields = line.split_whitespace();
        let (Some(label), Some(value)) = (fields.next(), fields.next()) else {
            continue;
        };
        let (Some(kind), Ok(value)) = (
            kinds().find(|kind| label.eq_ignore_ascii_case(kind)),
            value.parse::<u64>(),
        ) else {
            continue;
        };
        cost.entry(kind.to_owned()).or_insert(value);
    }

    let missing: Vec<&str> = kinds().filter(|kind| !cost.contains_key(*kind)).collect();
    if !missing.is_empty() {
        return Err(Error::IncompleteEmulatorReport {
            missing: missing.join(", "),
        });
    }

    // ZisK computes the total as exactly this sum, so parts that fall short of it mean the summary
    // block was misread rather than that the emulator disagrees with itself.
    let summed: u64 = KINDS.iter().map(|kind| cost[*kind]).sum();
    let total = cost[TOTAL];
    if summed != total {
        return Err(Error::UnexpectedCostKindsSum { summed, total });
    }

    Ok(cost)
}

#[cfg(test)]
mod tests {
    use crate::estimator::parse;

    /// A real `Stats::report` capture, taken through the same calls [`super::estimate_cost`] makes:
    /// the Nethermind ZisK guest run on mainnet block 25580033, with thousands separators turned
    /// off.
    const REPORT: &str = include_str!("testdata/ziskemu-report.txt");

    #[test]
    fn cost_is_read_from_the_report() {
        let cost = parse(REPORT).unwrap();
        assert_eq!(cost["main"], 5799084064);
        assert_eq!(cost["opcodes"], 1378944808);
        assert_eq!(cost["precompiles"], 1947067046);
        assert_eq!(cost["memory"], 766637533);
        assert_eq!(cost["base"], 293601280);
        assert_eq!(cost["total"], 10185334731);
        // Rows that are derived, re-counted or not costs at all stay out.
        for skipped in ["variable", "frops", "steps"] {
            assert!(!cost.contains_key(skipped), "{skipped} was recorded");
        }
    }

    /// Parts that fall short of the total mean the summary block was misread.
    #[test]
    fn a_report_that_does_not_sum_is_an_error() {
        let broken = REPORT.replacen("293601280", "293601281", 1);
        assert!(parse(&broken).is_err());
    }

    #[test]
    fn a_truncated_report_is_an_error() {
        assert!(parse("STEPS 1\nMAIN 2\n").is_err());
    }
}

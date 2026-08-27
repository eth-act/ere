use std::{
    collections::{BTreeMap, HashMap},
    env,
    ops::Range,
};

use ere_prover_core::{
    ERE_COST_ESTIMATION_HEAP_END, ERE_COST_ESTIMATION_HEAP_START, symbol_address,
};
use zisk_core::{RAM_ADDR, RAM_SIZE};

use crate::error::EstimateCostError;

const TOTAL: &str = "total";

/// Component names with their labels in the emulator report.
const COMPONENTS: [(&str, &str); 5] = [
    ("base", "base"),
    ("precompile", "precompiles"),
    ("memory", "memory"),
    ("opcode", "opcodes"),
    ("main", "main"),
];

const DEFAULT_HEAP_START: &str = "_heap_bottom";
const DEFAULT_HEAP_END: &str = "_heap_top";

/// Heap between the two heap symbols, when both exist and the range fits emulator RAM.
pub(crate) fn heap_range(elf: &[u8]) -> Option<Range<u64>> {
    let start =
        env::var(ERE_COST_ESTIMATION_HEAP_START).unwrap_or_else(|_| DEFAULT_HEAP_START.to_owned());
    let end =
        env::var(ERE_COST_ESTIMATION_HEAP_END).unwrap_or_else(|_| DEFAULT_HEAP_END.to_owned());
    let (start, end) = (symbol_address(elf, &start)?, symbol_address(elf, &end)?);
    (start < end && start >= RAM_ADDR && end <= RAM_ADDR + RAM_SIZE).then_some(start..end)
}

pub(crate) fn parse(report: &str) -> Result<BTreeMap<String, u64>, EstimateCostError> {
    let rows = rows(report);

    let missing: Vec<&str> = COMPONENTS
        .iter()
        .map(|(_, label)| *label)
        .chain([TOTAL])
        .filter(|label| !rows.contains_key(*label))
        .collect();
    if !missing.is_empty() {
        return Err(EstimateCostError::MissingRows(missing.join(", ")));
    }

    let cost: BTreeMap<String, u64> = COMPONENTS
        .iter()
        .map(|(component, label)| ((*component).to_owned(), rows[*label]))
        .collect();

    let (summed, total) = (cost.values().sum::<u64>(), rows[TOTAL]);
    if summed != total {
        return Err(EstimateCostError::Mismatch { summed, total });
    }

    Ok(cost)
}

/// Value after each line label, keyed by the lowercased label. Later sections reuse the summary
/// labels, so the first one wins.
fn rows(report: &str) -> HashMap<String, u64> {
    let mut rows = HashMap::new();
    for line in report.lines() {
        let mut fields = line.split_whitespace();
        let (Some(label), Some(value)) = (fields.next(), fields.next()) else {
            continue;
        };
        if let Ok(value) = value.parse() {
            rows.entry(label.to_ascii_lowercase()).or_insert(value);
        }
    }
    rows
}

pub(crate) fn peak_heap_bytes(bytes: &[u8]) -> Option<u64> {
    let highest = bytes.iter().rposition(|byte| *byte != 0)?;
    let lowest = bytes.iter().position(|byte| *byte != 0).unwrap();
    Some((highest - lowest + 1) as u64)
}

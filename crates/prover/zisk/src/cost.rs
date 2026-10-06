use std::collections::{BTreeMap, HashMap};

use crate::error::EstimateCostError;

mod profile;

pub(crate) use profile::{record_accesses, running_cost, stack_pointer_write};

const TOTAL: &str = "total";

/// Component names with their labels in the emulator report.
pub(crate) const COMPONENTS: [(&str, &str); 5] = [
    ("base", "base"),
    ("precompile", "precompiles"),
    ("memory", "memory"),
    ("opcode", "opcodes"),
    ("main", "main"),
];

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

use std::collections::BTreeMap;

use elf::{ElfBytes, endian::AnyEndian};

pub const ERE_COST_ESTIMATION_HEAP_START: &str = "ERE_COST_ESTIMATION_HEAP_START";
pub const ERE_COST_ESTIMATION_HEAP_END: &str = "ERE_COST_ESTIMATION_HEAP_END";

/// Cost and heap use of one execution.
#[derive(Clone, Debug, Default)]
pub struct CostEstimation {
    /// Cost per component. Each zkVM defines the unit.
    pub cost: BTreeMap<String, u64>,
    /// `None` if the estimator cannot read the heap.
    pub peak_heap_bytes: Option<u64>,
}

/// Address of `name` in the guest ELF, or `None` when the ELF carries no such symbol.
pub fn symbol_address(elf: &[u8], name: &str) -> Option<u64> {
    let elf = ElfBytes::<AnyEndian>::minimal_parse(elf).ok()?;
    let (symbols, names) = elf.symbol_table().ok()??;
    symbols
        .iter()
        .find(|symbol| {
            !symbol.is_undefined()
                && names
                    .get(symbol.st_name as usize)
                    .is_ok_and(|it| it == name)
        })
        .map(|symbol| symbol.st_value)
}

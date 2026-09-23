use std::{env, ops::Range};

use ere_prover_core::{
    ERE_COST_ESTIMATION_HEAP_END, ERE_COST_ESTIMATION_HEAP_START, symbol_address,
};

const DEFAULT_HEAP_START: &str = "_end";

/// Top of the guest heap.
///
/// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/syscalls/src/allocator.rs#L3.
const DEFAULT_HEAP_END: u64 = 0xC000_0000;

/// Heap between the heap start symbol and the heap end, which is a symbol when
/// `ERE_COST_ESTIMATION_HEAP_END` is set, and `DEFAULT_HEAP_END` otherwise.
pub(crate) fn heap_range(elf: &[u8]) -> Option<Range<u64>> {
    let start =
        env::var(ERE_COST_ESTIMATION_HEAP_START).unwrap_or_else(|_| DEFAULT_HEAP_START.to_owned());
    let start = symbol_address(elf, &start)?;
    let end = match env::var(ERE_COST_ESTIMATION_HEAP_END) {
        Ok(end) => symbol_address(elf, &end)?,
        Err(_) => DEFAULT_HEAP_END,
    };
    (start < end).then_some(start..end)
}

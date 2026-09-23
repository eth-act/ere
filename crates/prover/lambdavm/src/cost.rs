use std::{collections::BTreeMap, env, ops::Range, sync::Arc};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CostEstimation, ERE_COST_ESTIMATION_HEAP_START, PublicValues, symbol_address,
};
use lambda_vm_executor::vm::memory::Memory;
use lambda_vm_prover::count_elements;

use crate::{
    error::Error,
    executor::{extract_public_values, run},
};

const DEFAULT_HEAP_START: &str = "_end";

/// Top of the guest heap.
///
/// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/syscalls/src/allocator.rs#L3.
const HEAP_END: u64 = 0xC000_0000;

pub(crate) struct CostEstimator {
    elf: Elf,
    program: Arc<lambda_vm_executor::elf::Elf>,
    heap_range: Option<Range<u64>>,
}

impl CostEstimator {
    pub(crate) fn new(elf: &Elf, program: &Arc<lambda_vm_executor::elf::Elf>) -> Self {
        let start = env::var(ERE_COST_ESTIMATION_HEAP_START)
            .unwrap_or_else(|_| DEFAULT_HEAP_START.to_owned());
        let heap_range = symbol_address(&elf.0, &start)
            .filter(|start| *start < HEAP_END)
            .map(|start| start..HEAP_END);

        Self {
            elf: elf.clone(),
            program: program.clone(),
            heap_range,
        }
    }

    pub(crate) fn estimate(&self, stdin: &[u8]) -> Result<(PublicValues, CostEstimation), Error> {
        let (executor, cycles) = run(&self.program, stdin)?;

        let (main_elements, aux_elements) =
            count_elements(&self.elf.0, stdin).map_err(Error::EstimateCost)?;

        let cost = BTreeMap::from([
            ("cycles".to_owned(), cycles),
            ("main_elements".to_owned(), main_elements),
            ("aux_elements".to_owned(), aux_elements),
        ]);

        let peak_heap_bytes = self
            .heap_range
            .as_ref()
            .map(|range| peak_heap_bytes(range, executor.memory()));

        Ok((
            extract_public_values(executor)?,
            CostEstimation {
                cost,
                peak_heap_bytes,
            },
        ))
    }
}

/// Bytes from the heap start up to the highest non-zero heap byte, or `0` for an
/// unused heap.
///
/// Unlike the other provers, this scans the sparse memory map instead of a dense
/// slice, because a dense read of the heap range would allocate about 3 GiB.
fn peak_heap_bytes(range: &Range<u64>, memory: &Memory) -> u64 {
    memory
        .iter_bytes()
        .filter(|(address, byte)| range.contains(address) && *byte != 0)
        .map(|(address, _)| address + 1 - range.start)
        .max()
        .unwrap_or(0)
}

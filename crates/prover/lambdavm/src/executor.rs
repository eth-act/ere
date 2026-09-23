//! LambdaVM execution.

use std::ops::Range;

use ere_prover_core::PublicValues;
use lambda_vm_executor::{elf::Elf, vm::execution::Executor};

use crate::error::Error;

/// Execution stops with an error beyond this many cycles, so a guest that never halts cannot hang
/// the caller.
pub(crate) const MAX_CYCLES: u64 = 1 << 32;

/// Cycles run between two checks of [`MAX_CYCLES`].
const CHUNK_CYCLES: usize = 1 << 20;

/// Result of one execution.
pub(crate) struct Execution {
    pub(crate) public_values: PublicValues,
    pub(crate) cycles: u64,
    pub(crate) peak_heap_bytes: Option<u64>,
}

/// Runs `program` on `stdin`, and measures the heap in `heap_range` if given.
///
/// Runs in chunks and drops the logs of each chunk, so memory use does not grow with the cycle
/// count.
pub(crate) fn execute(
    program: &Elf,
    stdin: &[u8],
    heap_range: Option<&Range<u64>>,
) -> Result<Execution, Error> {
    let mut executor = Executor::new(program, stdin.to_vec())?;

    let mut cycles = 0;
    while let Some(logs) = executor.resume_with_limit(CHUNK_CYCLES)? {
        cycles += logs.len() as u64;
        if cycles > MAX_CYCLES {
            return Err(Error::CycleLimitExceeded(MAX_CYCLES));
        }
    }

    let peak_heap_bytes = heap_range.map(|range| {
        peak_heap_bytes(
            range,
            executor
                .memory()
                .iter_bytes()
                .filter(|(address, byte)| range.contains(address) && *byte != 0)
                .map(|(address, _)| address),
        )
    });

    let public_values = executor.finish()?.memory_values.into();

    Ok(Execution {
        public_values,
        cycles,
        peak_heap_bytes,
    })
}

/// Bytes from the heap start up to the highest non-zero heap byte, or `0` for an unused heap.
///
/// Unlike the dense helper of the other provers, this takes the addresses of the non-zero bytes
/// in any order, because LambdaVM memory is a sparse map, and reading the whole heap range densely
/// would allocate about 3 GiB.
fn peak_heap_bytes(heap_range: &Range<u64>, addresses: impl Iterator<Item = u64>) -> u64 {
    addresses
        .max()
        .map_or(0, |highest| highest + 1 - heap_range.start)
}

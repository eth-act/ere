use std::sync::Arc;

use sp1_core_executor::{GAS_TRACE_CHUNK_THRESHOLD, MinimalExecutorEnum, Program, TraceChunkRaw};
use sp1_jit::SyscallContext;

use crate::error::{Error, EstimateCostError};

const GUEST_WORD_BYTES: u64 = 8;

/// Executor for targets without the SP1 JIT.
pub(crate) struct Executor {
    program: Arc<Program>,
    heap_start: Option<u64>,
}

impl Executor {
    pub(crate) fn new(program: Arc<Program>, heap_start: Option<u64>) -> Self {
        Self {
            program,
            heap_start,
        }
    }

    pub(crate) fn execute(
        &self,
        input: &[u8],
        mut charge: impl FnMut(&TraceChunkRaw) -> Result<(), Error>,
    ) -> Result<(Option<u64>, Vec<u8>), Error> {
        let mut executor = MinimalExecutorEnum::new(
            Arc::clone(&self.program),
            false,
            Some(GAS_TRACE_CHUNK_THRESHOLD),
        );
        executor.with_input(input);

        while let Some(chunk) = executor
            .try_execute_chunk()
            .map_err(|err| EstimateCostError::Execute(err.to_string()))?
        {
            charge(&chunk)?;
        }

        // The memory image lives until `into_public_values_stream` consumes the executor.
        let peak = self
            .heap_start
            .and_then(|heap_start| peak_heap_bytes(&executor, heap_start, input));
        Ok((peak, executor.into_public_values_stream()))
    }
}

/// Counts the filled words above `heap_start`, so it does not match the span the other backends
/// report.
///
/// The runtime places the input above the heap, so every word below the input is heap. The cut
/// follows the input length, not the widest gap. A zero word inside the input splits it otherwise.
fn peak_heap_bytes(executor: &MinimalExecutorEnum, heap_start: u64, input: &[u8]) -> Option<u64> {
    let mut filled = filled_words(executor, heap_start);
    filled.sort_unstable();

    let input_bytes = GUEST_WORD_BYTES * (input.len() as u64).div_ceil(GUEST_WORD_BYTES);
    let input_start = filled.last()?.checked_sub(input_bytes)?;
    let heap_words = filled.partition_point(|address| *address < input_start);

    (heap_words > 0).then(|| GUEST_WORD_BYTES * heap_words as u64)
}

/// Every guest word at or above `heap_start` holding a value other than zero.
///
/// The executor inserts an entry on read as well as on write, so the address alone does not say
/// the guest put anything there.
fn filled_words(executor: &MinimalExecutorEnum, heap_start: u64) -> Vec<u64> {
    let addresses: Vec<u64> = match executor {
        MinimalExecutorEnum::Supervisor(executor) => {
            executor.init_addr_iter().into_iter().collect()
        }
        MinimalExecutorEnum::User(executor) => executor.init_addr_iter().into_iter().collect(),
    };
    addresses
        .into_iter()
        .filter(|address| *address >= heap_start)
        .filter(|address| executor.get_memory_value(*address).value != 0)
        .collect()
}

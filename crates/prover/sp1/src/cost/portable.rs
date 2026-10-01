use std::sync::Arc;

use sp1_core_executor::{GAS_TRACE_CHUNK_THRESHOLD, MinimalExecutorEnum, Program, TraceChunkRaw};

use crate::error::{Error, EstimateCostError};

/// Executor for targets without the SP1 JIT.
pub(crate) struct Executor {
    program: Arc<Program>,
}

impl Executor {
    pub(crate) fn new(program: Arc<Program>) -> Self {
        Self { program }
    }

    pub(crate) fn execute(
        &self,
        input: &[u8],
        mut charge: impl FnMut(&TraceChunkRaw) -> Result<(), Error>,
    ) -> Result<Vec<u8>, Error> {
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

        Ok(executor.into_public_values_stream())
    }
}

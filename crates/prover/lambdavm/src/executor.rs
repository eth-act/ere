//! LambdaVM execution instance.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use ere_prover_core::PublicValues;
use lambda_vm_executor::{elf::Elf, vm::execution::Executor as VmExecutor};

use crate::error::Error;

/// Execution stops with an error beyond this many cycles, so a guest that never
/// halts cannot hang the caller.
pub(crate) const MAX_CYCLES: u64 = 1 << 32;

/// Cycles run between two checks of [`MAX_CYCLES`].
const CHUNK_CYCLES: usize = 1 << 20;

/// An execution instance of a loaded program.
pub(crate) struct Executor {
    program: Arc<Elf>,
}

impl Executor {
    pub(crate) fn new(program: &Arc<Elf>) -> Self {
        Self {
            program: program.clone(),
        }
    }

    /// Runs `stdin` on the instance.
    pub(crate) fn execute(&self, stdin: &[u8]) -> Result<(PublicValues, Duration), Error> {
        let start = Instant::now();
        let (executor, _) = run(&self.program, stdin)?;
        let execution_duration = start.elapsed();

        Ok((extract_public_values(executor)?, execution_duration))
    }
}

/// Runs `program` on `stdin` until it halts, and returns the halted executor and
/// the cycle count.
///
/// Runs in chunks and drops the logs of each chunk, so memory use does not grow
/// with the cycle count.
pub(crate) fn run(program: &Elf, stdin: &[u8]) -> Result<(VmExecutor, u64), Error> {
    let mut executor = VmExecutor::new(program, stdin.to_vec()).map_err(Error::Execute)?;

    let mut cycles = 0;
    while let Some(logs) = executor
        .resume_with_limit(CHUNK_CYCLES)
        .map_err(Error::Execute)?
    {
        cycles += logs.len() as u64;
        if cycles > MAX_CYCLES {
            return Err(Error::CycleLimitExceeded(MAX_CYCLES));
        }
    }

    Ok((executor, cycles))
}

pub(crate) fn extract_public_values(executor: VmExecutor) -> Result<PublicValues, Error> {
    Ok(executor
        .finish()
        .map_err(Error::Execute)?
        .memory_values
        .into())
}

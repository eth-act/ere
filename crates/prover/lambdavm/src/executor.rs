//! LambdaVM execution instance.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use ere_prover_core::PublicValues;
use lambda_vm_executor::{elf::Elf, vm::execution::Executor as VmExecutor};

use crate::error::Error;

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
        let executor = run(&self.program, stdin)?;
        let execution_duration = start.elapsed();

        Ok((extract_public_values(executor)?, execution_duration))
    }
}

/// Runs `program` on `stdin` until it halts, and returns the halted executor.
///
/// Runs in chunks and drops the logs of each chunk, so memory use does not grow
/// with the cycle count.
pub(crate) fn run(program: &Elf, stdin: &[u8]) -> Result<VmExecutor, Error> {
    let mut executor = VmExecutor::new(program, stdin.to_vec()).map_err(Error::Execute)?;

    while executor.resume().map_err(Error::Execute)?.is_some() {}

    Ok(executor)
}

pub(crate) fn extract_public_values(executor: VmExecutor) -> Result<PublicValues, Error> {
    Ok(executor
        .finish()
        .map_err(Error::Execute)?
        .memory_values
        .into())
}

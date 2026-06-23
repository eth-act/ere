//! A bounded pool of reusable JIT executors.

use std::{
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    sync::Arc,
    thread,
    time::Instant,
};

use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, bounded};
use ere_prover_core::{ProgramExecutionReport, PublicValues};
use sp1_core_executor::{MinimalExecutorEnum, Program};
use sp1_sdk::{SP1Stdin, StatusCode};

use crate::error::Error;

/// A fixed-size pool of executors. Size is bounded by host's available parallelism.
pub(crate) struct SP1ExecutorPool {
    rx: Receiver<MinimalExecutorEnum>,
    tx: Sender<MinimalExecutorEnum>,
}

impl SP1ExecutorPool {
    pub(crate) fn new(elf: &[u8]) -> Result<Self, Error> {
        let program = Program::from(elf)
            .map_err(|err| Error::setup(anyhow!("failed to disassemble program: {err}")))?
            .into();
        let size = execution_concurrency();
        let (tx, rx) = bounded(size);
        for _ in 0..size {
            tx.send(MinimalExecutorEnum::new(Arc::clone(&program), false, None))
                .unwrap();
        }
        Ok(Self { rx, tx })
    }

    /// Runs `stdin` on an rx executor, blocking until one is free. The
    /// executor rejoins the pool once the run completes.
    pub(crate) fn execute(
        &self,
        stdin: SP1Stdin,
    ) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        let mut executor = ExecutorGuard {
            executor: Some(self.rx.recv().unwrap()),
            tx: &self.tx,
        };

        let SP1Stdin { buffer, .. } = stdin;

        let start = Instant::now();
        executor.reset();
        for chunk in &buffer {
            executor.with_input(chunk);
        }
        while !executor.is_done() {
            executor.execute_chunk();
        }
        let execution_duration = start.elapsed();

        let exit_code = executor.exit_code();
        if exit_code != StatusCode::SUCCESS.as_u32() {
            return Err(Error::ExecutionFailed(exit_code));
        }

        let public_values = executor.public_values_stream().as_slice().into();
        let total_num_cycles = executor.global_clk();

        drop(executor);

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles,
                execution_duration,
                ..Default::default()
            },
        ))
    }
}

/// An executor borrowed from an [`SP1ExecutorPool`], returned to it on drop.
pub(crate) struct ExecutorGuard<'a> {
    executor: Option<MinimalExecutorEnum>,
    tx: &'a Sender<MinimalExecutorEnum>,
}

impl Deref for ExecutorGuard<'_> {
    type Target = MinimalExecutorEnum;

    fn deref(&self) -> &Self::Target {
        self.executor.as_ref().unwrap()
    }
}

impl DerefMut for ExecutorGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.executor.as_mut().unwrap()
    }
}

impl Drop for ExecutorGuard<'_> {
    fn drop(&mut self) {
        if let Some(executor) = self.executor.take() {
            let _ = self.tx.send(executor);
        }
    }
}

/// The executor pool size, bounding concurrent executions to the host's
/// available parallelism.
fn execution_concurrency() -> usize {
    thread::available_parallelism().map_or(1, NonZeroUsize::get)
}

//! A bounded pool of reusable SP1 execution instances.

use std::{
    env,
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

/// Upper bound on the pool size when derived from available parallelism.
const MAX_POOL_SIZE: usize = 32;

/// A fixed-size pool of reusable SP1 execution instances.
///
/// The size defaults to the host's available parallelism capped by
/// [`MAX_POOL_SIZE`], and the `ERE_SP1_EXECUTOR_POOL_SIZE` environment variable
/// overrides the bound.
pub(crate) struct SP1ExecutorPool {
    program: Arc<Program>,
    rx: Receiver<MinimalExecutorEnum>,
    tx: Sender<MinimalExecutorEnum>,
}

impl SP1ExecutorPool {
    pub(crate) fn new(elf: &[u8]) -> Result<Self, Error> {
        let program: Arc<Program> = Program::from(elf)
            .map_err(|err| Error::setup(anyhow!("failed to disassemble program: {err}")))?
            .into();
        let size = execution_concurrency();
        let (tx, rx) = bounded(size);
        for _ in 0..size {
            tx.send(MinimalExecutorEnum::new(Arc::clone(&program), false, None))
                .unwrap();
        }
        Ok(Self { program, rx, tx })
    }

    /// The disassembled program the pooled executors run.
    pub(crate) fn program(&self) -> &Arc<Program> {
        &self.program
    }

    /// Runs `stdin` on a pooled executor, blocking until one is free. The
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
            return Err(Error::ExecutionFailed(exit_code.into()));
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

/// The executor pool size, bounding concurrent executions.
///
/// Defaults to the host's available parallelism capped by [`MAX_POOL_SIZE`].
/// `ERE_SP1_EXECUTOR_POOL_SIZE` overrides the bound with an explicit size.
fn execution_concurrency() -> usize {
    env::var("ERE_SP1_EXECUTOR_POOL_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&size| size > 0)
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map_or(1, NonZeroUsize::get)
                .min(MAX_POOL_SIZE)
        })
}

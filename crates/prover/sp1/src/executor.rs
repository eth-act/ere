//! Bounded concurrent SP1 execution.

use std::{
    env,
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, bounded};
use ere_prover_core::PublicValues;
use sp1_core_executor::{MinimalExecutorEnum, Program};
use sp1_sdk::{SP1Stdin, StatusCode};

use crate::error::Error;

/// Upper bound on the concurrency derived from available parallelism.
const MAX_CONCURRENCY: usize = 32;

/// Runs a program on a fixed set of reusable execution instances.
pub(crate) struct SP1Executor {
    rx: Receiver<MinimalExecutorEnum>,
    tx: Sender<MinimalExecutorEnum>,
    program: Arc<Program>,
}

impl SP1Executor {
    pub(crate) fn new(elf: &[u8]) -> Result<Self, Error> {
        let program: Arc<Program> = Program::from(elf)
            .map_err(|err| Error::setup(anyhow!("failed to disassemble program: {err}")))?
            .into();
        let concurrency = execution_concurrency();
        let (tx, rx) = bounded(concurrency);
        for _ in 0..concurrency {
            tx.send(MinimalExecutorEnum::new(Arc::clone(&program), false, None))
                .unwrap();
        }
        Ok(Self { rx, tx, program })
    }

    pub(crate) fn program(&self) -> Arc<Program> {
        Arc::clone(&self.program)
    }

    /// Runs `stdin`, blocking until an instance is free.
    pub(crate) fn execute(&self, stdin: SP1Stdin) -> Result<(PublicValues, Duration), Error> {
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

        drop(executor);

        Ok((public_values, execution_duration))
    }
}

/// An instance borrowed from an [`SP1Executor`], returned to it on drop.
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

/// Executions that may run at once, which `ERE_SP1_EXECUTOR_CONCURRENCY` states outright.
pub(crate) fn execution_concurrency() -> usize {
    env::var("ERE_SP1_EXECUTOR_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&concurrency| concurrency > 0)
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map_or(1, NonZeroUsize::get)
                .min(MAX_CONCURRENCY)
        })
}

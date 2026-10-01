use std::{env, mem, sync::Arc};

use crossbeam_channel::{Receiver, Sender, bounded};
use memmap2::MmapMut;
use sp1_core_executor::{
    GAS_TRACE_CHUNK_THRESHOLD, HALT_PC, MinimalTranspiler, Program, TraceChunkRaw,
};
use sp1_jit::{JitFunction, memory::AnonymousMemory, trace_capacity};
use sp1_primitives::consts::MAX_JIT_LOG_ADDR;
use sysinfo::System;

use crate::{
    error::{Error, EstimateCostError},
    executor::execution_concurrency,
};

type Jit = JitFunction<AnonymousMemory>;

pub(crate) struct Executor {
    program: Arc<Program>,
    permit_rx: Receiver<()>,
    permit_tx: Sender<()>,
}

impl Executor {
    pub(crate) fn new(program: Arc<Program>) -> Self {
        let concurrency = concurrency();
        let (permit_tx, permit_rx) = bounded(concurrency);
        for _ in 0..concurrency {
            permit_tx.send(()).unwrap();
        }
        Self {
            program,
            permit_rx,
            permit_tx,
        }
    }

    pub(crate) fn execute(
        &self,
        input: &[u8],
        mut charge: impl FnMut(&TraceChunkRaw) -> Result<(), Error>,
    ) -> Result<Vec<u8>, Error> {
        // Drops last, after the instance frees its guest memory.
        let _permit = self.acquire();
        let mut jit = self.transpile();

        jit.push_input(input.to_vec());

        let capacity = trace_capacity(Some(GAS_TRACE_CHUNK_THRESHOLD));
        while jit.pc != HALT_PC {
            let mut trace = MmapMut::map_anon(capacity).map_err(EstimateCostError::Memory)?;
            // SAFETY: the buffer has the capacity the transpiler writes into, and the chunk reads
            // it back in the executor's own layout.
            let chunk = unsafe {
                jit.call(trace.as_mut_ptr());
                TraceChunkRaw::new(trace.make_read_only().map_err(EstimateCostError::Memory)?)
            };
            charge(&chunk)?;
        }

        Ok(mem::take(&mut jit.public_values_stream))
    }

    /// Takes one of the permits that bound concurrent runs, blocking until one is free.
    fn acquire(&self) -> PermitGuard<'_> {
        self.permit_rx.recv().unwrap();
        PermitGuard {
            tx: &self.permit_tx,
        }
    }

    /// A new `Jit` per run, because `JitFunction::reset` maps a replacement guest memory before
    /// dropping the old one.
    fn transpile(&self) -> Jit {
        let transpiler = MinimalTranspiler::new(
            1usize << MAX_JIT_LOG_ADDR,
            false,
            Some(GAS_TRACE_CHUNK_THRESHOLD),
        );
        let mut jit = transpiler.transpile(&self.program);
        jit.with_initial_memory_image(self.program.memory_image.clone());
        jit
    }
}

/// A permit borrowed from an [`Executor`], returned to it on drop.
struct PermitGuard<'a> {
    tx: &'a Sender<()>,
}

impl Drop for PermitGuard<'_> {
    fn drop(&mut self) {
        let _ = self.tx.send(());
    }
}

/// Estimates that may run at once, which `ERE_SP1_EXECUTE_ESTIMATED_CONCURRENCY` states outright.
///
/// Absent that, an estimate holds a trace buffer where a plain execution holds none, so free memory
/// bounds the count as well as the core count.
fn concurrency() -> usize {
    if let Some(stated) = env::var("ERE_SP1_EXECUTE_ESTIMATED_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&concurrency| concurrency > 0)
    {
        return stated;
    }
    let per_run = trace_capacity(Some(GAS_TRACE_CHUNK_THRESHOLD)) as u64;
    let fits = (available_bytes() / per_run).max(1) as usize;
    execution_concurrency().min(fits)
}

/// Free bytes of the cgroup this process runs in, or of the host when it has no limit.
fn available_bytes() -> u64 {
    let mut system = System::new();
    system.refresh_memory();
    system
        .cgroup_limits()
        .map_or_else(|| system.available_memory(), |limits| limits.free_memory)
}

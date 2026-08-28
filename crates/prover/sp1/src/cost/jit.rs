use std::{env, mem, ops::Range, os::fd::AsRawFd, sync::Arc};

use crossbeam_channel::{Receiver, Sender, bounded};
use memmap2::MmapMut;
use sp1_core_executor::{
    GAS_TRACE_CHUNK_THRESHOLD, HALT_PC, MinimalTranspiler, Program, TraceChunkRaw,
};
use sp1_jit::{JitFunction, memory::AnonymousMemory, trace_capacity};
use sp1_primitives::consts::MAX_JIT_LOG_ADDR;
use sysinfo::System;
use tracing::warn;

use crate::{
    error::{Error, EstimateCostError},
    executor::execution_concurrency,
};

/// Granularity the file behind guest memory reports data at, which is the page size of the host
/// this executor builds for. Nothing about a guest allocator is read through it.
const HOST_PAGE_BYTES: u64 = 4096;

const INPUT_PREFIX_BYTES: usize = 64;

/// Word the runtime rounds an input up to before it reads the input in.
const GUEST_WORD_BYTES: u64 = 8;

/// Each guest word takes a 16 byte record, the clock then the value.
const FILE_BYTES_PER_GUEST_BYTE: u64 = 2;

type Jit = JitFunction<AnonymousMemory>;

pub(crate) struct Executor {
    program: Arc<Program>,
    heap_start: Option<u64>,
    permit_rx: Receiver<()>,
    permit_tx: Sender<()>,
}

impl Executor {
    pub(crate) fn new(program: Arc<Program>, heap_start: Option<u64>) -> Self {
        let concurrency = concurrency();
        let (permit_tx, permit_rx) = bounded(concurrency);
        for _ in 0..concurrency {
            permit_tx.send(()).unwrap();
        }
        Self {
            program,
            heap_start,
            permit_rx,
            permit_tx,
        }
    }

    pub(crate) fn execute(
        &self,
        input: &[u8],
        mut charge: impl FnMut(&TraceChunkRaw) -> Result<(), Error>,
    ) -> Result<(Option<u64>, Vec<u8>), Error> {
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

        // A heap that cannot be measured is reported as `None`, the same as every other backend.
        let peak = self.heap_start.and_then(|heap_start| {
            peak_heap_bytes(jit.memory.as_raw_fd(), heap_start, input)
                .inspect_err(|err| warn!("cannot measure the peak heap: {err}"))
                .ok()
                .flatten()
        });
        Ok((peak, mem::take(&mut jit.public_values_stream)))
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

/// Peak heap in bytes, from the pages the file behind guest memory holds.
///
/// The heap starts at the linker symbol the caller resolved, and the file skips untouched pages, so
/// what the file holds above that symbol is what the guest reached. Where the input landed decides
/// where the heap ends, and that follows the allocator the guest links rather than the runtime.
fn peak_heap_bytes(
    memory_fd: i32,
    heap_start: u64,
    input: &[u8],
) -> Result<Option<u64>, EstimateCostError> {
    let mut written = Vec::new();
    let mut position = FILE_BYTES_PER_GUEST_BYTE * heap_start;
    while let Some(data) = seek(memory_fd, position, libc::SEEK_DATA)? {
        let hole =
            seek(memory_fd, data, libc::SEEK_HOLE)?.expect("data at an offset has a hole past it");
        written.push(data..hole);
        position = hole;
    }

    // The topmost region holds the input where the allocator reads it into memory reserved above
    // the heap, and it holds whatever that allocator keeps at the top of its own arena alongside
    // it. Neither is an allocation the guest made, so the heap is what was written below them. An
    // allocator that allocates the input instead leaves that region holding heap like any other.
    let Some(top) = written.last().cloned() else {
        return Ok(None);
    };
    if input_file_offset(memory_fd, &top, input)?.is_some() {
        written.pop();
    }

    let (Some(first), Some(last)) = (written.first(), written.last()) else {
        return Ok(None);
    };
    Ok(Some(
        (last.end - 1) / FILE_BYTES_PER_GUEST_BYTE - first.start / FILE_BYTES_PER_GUEST_BYTE + 1,
    ))
}

/// File offset the input starts at inside `region`, or `None` where the region holds no input.
///
/// A reserved input region starts on a page and holds the input rounded up to a whole number of
/// guest words, so the input is looked for a page at a time.
fn input_file_offset(
    memory_fd: i32,
    region: &Range<u64>,
    input: &[u8],
) -> Result<Option<u64>, EstimateCostError> {
    let padded = (input.len() as u64).next_multiple_of(GUEST_WORD_BYTES);
    let Some(last) = region
        .end
        .checked_sub(FILE_BYTES_PER_GUEST_BYTE * padded)
        .filter(|last| *last >= region.start)
    else {
        return Ok(None);
    };
    // An empty input is nowhere, and looking for it would match the first page read.
    let prefix = &input[..INPUT_PREFIX_BYTES.min(input.len())];
    if prefix.is_empty() {
        return Ok(None);
    }
    (region.start..=last)
        .step_by(HOST_PAGE_BYTES as usize)
        .map(|at| read_guest_bytes(memory_fd, at).map(|bytes| (at, bytes)))
        .find_map(|read| match read {
            Ok((at, bytes)) => (bytes[..prefix.len()] == *prefix).then_some(Ok(at)),
            Err(err) => Some(Err(err)),
        })
        .transpose()
}

/// Guest bytes at the word-aligned file offset `at`.
fn read_guest_bytes(
    memory_fd: i32,
    at: u64,
) -> Result<[u8; INPUT_PREFIX_BYTES], EstimateCostError> {
    let mut records = [0u8; 2 * INPUT_PREFIX_BYTES];
    // SAFETY: the caller holds the mapping that owns this descriptor, and the buffer fits every
    // byte the read can write.
    let read = unsafe {
        libc::pread(
            memory_fd,
            records.as_mut_ptr().cast(),
            records.len(),
            at as i64,
        )
    };
    if read != records.len() as isize {
        return Err(EstimateCostError::Memory(std::io::Error::last_os_error()));
    }
    let mut guest_bytes = [0u8; INPUT_PREFIX_BYTES];
    let (records, _) = records.as_chunks::<16>();
    for (bytes, record) in guest_bytes.chunks_mut(8).zip(records) {
        bytes.copy_from_slice(&record[8..]);
    }
    Ok(guest_bytes)
}

/// Estimates that may run at once, which `ERE_SP1_ESTIMATOR_CONCURRENCY` states outright.
///
/// Absent that, an estimate holds a trace buffer where a plain execution holds none, so free memory
/// bounds the count as well as the core count.
fn concurrency() -> usize {
    if let Some(stated) = env::var("ERE_SP1_ESTIMATOR_CONCURRENCY")
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

/// Reports the end of the data as `None` instead of an error.
fn seek(memory_fd: i32, offset: u64, whence: i32) -> Result<Option<u64>, EstimateCostError> {
    // SAFETY: the caller holds the mapping that owns this descriptor, and a file with no data
    // past `offset` reports ENXIO.
    let found = unsafe { libc::lseek(memory_fd, offset as i64, whence) };
    match found {
        0.. => Ok(Some(found as u64)),
        _ => match std::io::Error::last_os_error() {
            err if err.raw_os_error() == Some(libc::ENXIO) => Ok(None),
            err => Err(EstimateCostError::Memory(err)),
        },
    }
}

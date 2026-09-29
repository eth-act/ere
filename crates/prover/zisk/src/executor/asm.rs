//! Execution on the ZisK ASM emulator in Fast mode.

use std::{
    array, env,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    os::unix::fs::{FileExt, symlink},
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::{self, Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use ere_prover_core::{CommonError, PublicValues};
use once_cell::sync::OnceCell;
use parking_lot::{Condvar, Mutex};
use zisk_common::ZISK_PUBLICS;
use zisk_core::{
    AsmGenerationMethod, INPUT_ADDR, MAX_INPUT_SIZE, RAM_ADDR, RAM_SIZE, ZiskRom, ZiskRom2Asm,
};
use zisk_rom_setup::{ensure_ziskclib, get_elf_data_hash, get_output_path, resolve_emulator_asm};

use crate::{
    error::Error,
    sdk::{MAX_STEPS, panic_msg},
};

// Request and response types of `emulator-asm/src/constants.hpp`.
const TYPE_PING: u64 = 1;
const TYPE_PONG: u64 = 2;
const TYPE_FA_REQUEST: u64 = 13;
const TYPE_FA_RESPONSE: u64 = 14;

/// Linker script command of the Makefile that reserves the addresses of the input, the ROM, the RAM
/// and the MT and MO trace.
const TRACE_RESERVATION: &str = ". = . + 0x890000000;";

/// Chunk size of a Fast request. The service requires it and the step limit to be powers of two.
const CHUNK_SIZE: u64 = 1 << 18;

/// Start of the stderr line that gives the error code of an emulation.
const ERROR_CODE_LINE: &str = "Emulation ended with error code ";

/// Part of the stderr metrics line before the number of steps.
const STEPS_FIELD: &str = ", steps = ";

/// Default timeout of the start of a service and of one execution. Fast mode has no step limit, so
/// the timeout ends a run that does not stop.
const DEFAULT_ZISK_EXECUTE_TIMEOUT_SECS: u64 = 300;

/// Upper bound on the concurrency derived from available parallelism.
const MAX_CONCURRENCY: usize = 32;

/// Number of services started in this process, which makes each shared memory prefix unique.
static SERVICE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Runs a program on a pool of ASM emulator services. The pool starts services as concurrent
/// executions need them.
pub(crate) struct AsmExecutor {
    elf_hash: String,
    execute_timeout: Duration,
    binary_path: OnceCell<PathBuf>,
    max_size: usize,
    pool: Mutex<Pool>,
    cond: Condvar,
}

struct Pool {
    idle: Vec<Service>,
    num_services: usize,
}

impl AsmExecutor {
    pub(crate) fn new(elf: &[u8]) -> Self {
        Self {
            elf_hash: get_elf_data_hash(elf),
            execute_timeout: Duration::from_secs(
                env::var("ERE_ZISK_EXECUTE_TIMEOUT_SECS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(DEFAULT_ZISK_EXECUTE_TIMEOUT_SECS),
            ),
            binary_path: OnceCell::new(),
            max_size: execute_concurrency(),
            pool: Mutex::new(Pool {
                idle: Vec::new(),
                num_services: 0,
            }),
            cond: Condvar::new(),
        }
    }

    /// Runs the framed `stdin` on the program of `rom`, blocking until a service is free.
    pub(crate) fn execute(
        &self,
        rom: &ZiskRom,
        stdin: &[u8],
    ) -> Result<(PublicValues, Duration), Error> {
        self.get(rom)?.execute(stdin, self.execute_timeout)
    }

    /// Takes the most recently used idle service, starts a new one if the pool is not full, or
    /// waits for one to return.
    fn get(&self, rom: &ZiskRom) -> Result<PooledService<'_>, Error> {
        let mut pool = self.pool.lock();
        loop {
            if let Some(mut service) = pool.idle.pop() {
                if service.has_broken() {
                    pool.num_services -= 1;
                    drop(pool);
                    drop(service);
                    pool = self.pool.lock();
                    continue;
                }
                return Ok(PooledService {
                    executor: self,
                    service: Some(service),
                });
            }
            if pool.num_services < self.max_size {
                pool.num_services += 1;
                drop(pool);
                // Until it holds the new service, the guard frees the place when the start fails.
                let mut pooled = PooledService {
                    executor: self,
                    service: None,
                };
                let binary_path = self.binary_path.get_or_try_init(|| {
                    panic::catch_unwind(AssertUnwindSafe(|| build(rom, &self.elf_hash)))
                        .map_err(|err| Error::BuildAsmEmulator(anyhow!(panic_msg(err))))?
                })?;
                pooled.service = Some(Service::spawn(binary_path, self.execute_timeout)?);
                return Ok(pooled);
            }
            self.cond.wait(&mut pool);
        }
    }
}

/// A service taken from an [`AsmExecutor`], returned to it on drop.
struct PooledService<'a> {
    executor: &'a AsmExecutor,
    service: Option<Service>,
}

impl Deref for PooledService<'_> {
    type Target = Service;

    fn deref(&self) -> &Service {
        self.service.as_ref().unwrap()
    }
}

impl DerefMut for PooledService<'_> {
    fn deref_mut(&mut self) -> &mut Service {
        self.service.as_mut().unwrap()
    }
}

impl Drop for PooledService<'_> {
    /// Returns the service to the pool, or frees its place when no service started or it broke.
    fn drop(&mut self) {
        let mut service = self.service.take();
        let idle = service.take_if(|service| !service.has_broken());
        let mut pool = self.executor.pool.lock();
        match idle {
            Some(idle) => pool.idle.push(idle),
            None => pool.num_services -= 1,
        }
        drop(pool);
        drop(service);
        self.executor.cond.notify_one();
    }
}

/// Executions that may run at once, which `ERE_ZISK_EXECUTE_CONCURRENCY` states outright.
fn execute_concurrency() -> usize {
    env::var("ERE_ZISK_EXECUTE_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&concurrency| concurrency > 0)
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map_or(1, NonZeroUsize::get)
                .min(MAX_CONCURRENCY)
        })
}

/// Builds the Fast binary of the program into the ZisK cache, unless it is there.
fn build(rom: &ZiskRom, elf_hash: &str) -> Result<PathBuf, Error> {
    let cache_dir = get_output_path(&None).map_err(Error::BuildAsmEmulator)?;
    // Keyed by the ELF hash, like the MT, RH and MO binaries of `zisk_rom_setup`.
    let binary_path = cache_dir.join(format!("{elf_hash}-ft.bin"));
    if binary_path.exists() {
        return Ok(binary_path);
    }

    let (emulator_asm_dir, source) = resolve_emulator_asm().map_err(Error::BuildAsmEmulator)?;
    ensure_ziskclib(&emulator_asm_dir, source).map_err(Error::BuildAsmEmulator)?;

    // The Makefile writes a fixed `build` directory that other builds share, so the build runs in a
    // private copy of the `emulator-asm` layout. The binary moves into the cache only when
    // complete.
    let tempdir = tempfile::tempdir_in(&cache_dir).map_err(CommonError::tempdir)?;
    let build_dir = tempdir.path().join("zisk/emulator-asm");
    fs::create_dir_all(&build_dir)
        .map_err(|err| CommonError::create_dir("ASM emulator build", &build_dir, err))?;
    for (original, link) in [
        (
            emulator_asm_dir.join("Makefile"),
            build_dir.join("Makefile"),
        ),
        (emulator_asm_dir.join("src"), build_dir.join("src")),
        (
            emulator_asm_dir.join("../target"),
            tempdir.path().join("zisk/target"),
        ),
        (
            emulator_asm_dir.join("../../bin"),
            tempdir.path().join("bin"),
        ),
    ] {
        symlink(original, &link)
            .map_err(|err| CommonError::write_file("ASM emulator build", &link, err))?;
    }

    let asm_path = tempdir.path().join("ft.asm");
    let temp_binary_path = tempdir.path().join("ft.bin");
    ZiskRom2Asm::save_to_asm_file(
        rom,
        &asm_path,
        AsmGenerationMethod::AsmFast,
        false,
        false,
        false,
    );

    // Fast mode maps nothing past the RAM. Reserving the trace address space too makes exec fail on
    // hosts with less memory.
    make(&build_dir, &["build/zisk.ld".to_string()])?;
    let linker_script_path = build_dir.join("build/zisk.ld");
    let linker_script = fs::read_to_string(&linker_script_path)
        .map_err(|err| CommonError::read_file("linker script", &linker_script_path, err))?;
    if !linker_script.contains(TRACE_RESERVATION) {
        return Err(Error::BuildAsmEmulator(anyhow!(
            "{} lacks `{TRACE_RESERVATION}`",
            linker_script_path.display()
        )));
    }
    let reservation = format!(". = . + {:#x};", RAM_ADDR + RAM_SIZE - INPUT_ADDR);
    fs::write(
        &linker_script_path,
        linker_script.replace(TRACE_RESERVATION, &reservation),
    )
    .map_err(|err| CommonError::write_file("linker script", &linker_script_path, err))?;

    make(
        &build_dir,
        &[
            format!("EMU_PATH={}", asm_path.display()),
            format!("OUT_PATH={}", temp_binary_path.display()),
            "TRACE_TARGET=NONE".to_string(),
        ],
    )?;

    fs::rename(&temp_binary_path, &binary_path)
        .map_err(|err| CommonError::write_file("ASM emulator", &binary_path, err))?;

    Ok(binary_path)
}

fn make(build_dir: &Path, args: &[String]) -> Result<(), Error> {
    let mut command = Command::new("make");
    command.args(args).current_dir(build_dir);
    let output = command
        .output()
        .map_err(|err| CommonError::command(&command, err))?;
    if !output.status.success() {
        Err(CommonError::command_exit_non_zero(
            &command,
            output.status,
            Some(&output),
        ))?
    }
    Ok(())
}

/// Spawns `command` from a thread that lives as long as the process. The service sets
/// `PR_SET_PDEATHSIG`, which kills it when the spawning thread ends.
fn spawn(command: Command) -> Result<Child, Error> {
    type Request = (Command, Sender<Result<Child, Error>>);
    static SPAWNER: LazyLock<Sender<Request>> = LazyLock::new(|| {
        let (sender, receiver) = mpsc::channel::<Request>();
        thread::spawn(move || {
            for (mut command, reply) in receiver {
                let child = command
                    .spawn()
                    .map_err(|err| CommonError::command(&command, err).into());
                let _ = reply.send(child);
            }
        });
        sender
    });
    let (reply, child) = mpsc::channel();
    SPAWNER.send((command, reply)).unwrap();
    child.recv().unwrap()
}

/// An ASM emulator service in Fast mode, which runs one request at a time.
struct Service {
    child: Child,
    pipes: Pipes,
    input: File,
    control_input: File,
}

impl Service {
    /// Starts a service, and kills it when it does not answer within `timeout`.
    fn spawn(binary_path: &Path, timeout: Duration) -> Result<Self, Error> {
        let shm_prefix = format!(
            "ZISK_{}_ft{}",
            process::id(),
            SERVICE_COUNT.fetch_add(1, Ordering::Relaxed)
        );

        let mut command = Command::new(binary_path);
        command
            .args(["-s", "--gen=0", "--stdio", "-o", "-m", "--silent", "-u"])
            .args(["--shm_prefix", &shm_prefix])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = spawn(command)?;
        let mut pipes = Pipes {
            stdin: child.stdin.take().unwrap(),
            stdout: child.stdout.take().unwrap(),
            stderr: BufReader::new(child.stderr.take().unwrap()),
            broken: false,
        };

        // The service creates its shared memory before it answers. Unlinking the names now lets the
        // kernel free the memory when both processes end, however they end.
        let files = kill_on_timeout(&mut child, timeout, || {
            pipes.request([TYPE_PING, 0, 0, 0, 0], TYPE_PONG)?;
            let open = |name| {
                let path = format!("/dev/shm/{shm_prefix}_FT_{name}");
                File::options().write(true).open(&path).map_err(|err| {
                    CommonError::write_file("ASM emulator shared memory", &path, err)
                })
            };
            Ok((open("input")?, open("control_input")?))
        });
        remove_shared_memory(&shm_prefix);
        let (input, control_input) = files.inspect_err(|_| {
            let _ = child.kill();
            let _ = child.wait();
        })?;

        Ok(Self {
            child,
            pipes,
            input,
            control_input,
        })
    }

    /// Whether a request failed or the service exited.
    fn has_broken(&mut self) -> bool {
        self.pipes.broken || matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Runs the framed `stdin`, and kills the service when the request takes longer than `timeout`.
    fn execute(
        &mut self,
        stdin: &[u8],
        timeout: Duration,
    ) -> Result<(PublicValues, Duration), Error> {
        let Self {
            child,
            pipes,
            input,
            control_input,
        } = self;
        let (response, execution_duration, (steps, log, public_values)) =
            kill_on_timeout(child, timeout, || {
                // Waits for the memory reset that follows the previous response, so the timing
                // excludes it.
                pipes.request([TYPE_PING, 0, 0, 0, 0], TYPE_PONG)?;
                // The idle service reads no input, so the file can shrink. The shrink drops the
                // rest of the previous input, and a read past the new input finds zeros, as in a
                // new service.
                input
                    .set_len(8 + stdin.len() as u64)
                    .and_then(|()| input.set_len(MAX_INPUT_SIZE))
                    .map_err(|err| CommonError::io("Clear ASM emulator input", err))?;
                let write = |file: &File, data: &[u8], offset| {
                    file.write_all_at(data, offset)
                        .map_err(|err| CommonError::io("Write ASM emulator shared memory", err))
                };
                let start = Instant::now();
                write(input, &[0; 8], 0)?;
                write(input, stdin, 8)?;
                // The control input fields are the precompile size, the exit flag, the input size
                // and the reset flag. With the reset flag set, a read past the input fails after
                // one 5 s wait.
                let control = [0, 0, stdin.len() as u64, 1].map(u64::to_le_bytes).concat();
                write(control_input, &control, 0)?;
                let response = pipes.request(
                    [TYPE_FA_REQUEST, MAX_STEPS, CHUNK_SIZE, 0, 0],
                    TYPE_FA_RESPONSE,
                )?;
                let execution_duration = start.elapsed();
                Ok((response, execution_duration, pipes.read_output()?))
            })?;

        if steps > MAX_STEPS {
            return Err(Error::EmulatorNotTerminated);
        }
        let ended = response[1] == 0;
        let error = log.iter().any(|line| line.starts_with(ERROR_CODE_LINE));
        match (ended, error) {
            (true, false) => Ok((public_values.into(), execution_duration)),
            (true, true) => Err(Error::EmulatorError),
            (false, _) => Err(Error::EmulatorPanic(log.join(", "))),
        }
    }
}

impl Drop for Service {
    /// Kills the service. Its shared memory names are already unlinked, so the kill loses nothing,
    /// and it also stops a service that does not answer.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs `f`, and kills and reaps `child` when `f` takes longer than `timeout`. The kill closes the
/// pipes of the child, which ends a read or write of `f` that waits for it.
fn kill_on_timeout<T>(
    child: &mut Child,
    timeout: Duration,
    f: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    thread::scope(|scope| {
        let (finished, finish) = mpsc::channel::<()>();
        let watchdog = scope.spawn(move || {
            let timed_out = finish.recv_timeout(timeout) == Err(RecvTimeoutError::Timeout);
            if timed_out {
                let _ = child.kill();
                let _ = child.wait();
            }
            timed_out
        });
        let result = f();
        drop(finished);
        if watchdog.join().unwrap() {
            return Err(Error::AsmEmulatorTimeout(timeout));
        }
        result
    })
}

/// Stdio of a service. Requests and responses are five `u64` each, and the log and output of a
/// request go to stderr before its response.
struct Pipes {
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: BufReader<ChildStderr>,
    /// Set when a request fails, after which the service answers no more requests.
    broken: bool,
}

impl Pipes {
    fn request(&mut self, request: [u64; 5], response_type: u64) -> Result<[u64; 5], Error> {
        let mut bytes = [0; 40];
        self.stdin
            .write_all(&request.map(u64::to_le_bytes).concat())
            .and_then(|()| self.stdout.read_exact(&mut bytes))
            .map_err(|err| self.exit_error(err))?;
        let response: [u64; 5] =
            array::from_fn(|i| u64::from_le_bytes(bytes[8 * i..8 * (i + 1)].try_into().unwrap()));
        // Some failures print to stdout before the service exits.
        if response[0] != response_type {
            let err = io::Error::new(io::ErrorKind::InvalidData, String::from_utf8_lossy(&bytes));
            self.broken = true;
            return Err(Error::AsmEmulatorFailed(err, String::new()));
        }
        Ok(response)
    }

    /// Reads the steps, the log lines and the public values that precede the response on stderr.
    /// Each hex line is one little-endian `u32` of the public values.
    fn read_output(&mut self) -> Result<(u64, Vec<String>, Vec<u8>), Error> {
        let mut steps = 0;
        let mut log = Vec::new();
        let mut public_values = Vec::with_capacity(4 * ZISK_PUBLICS);
        let mut line = String::new();
        while public_values.len() < 4 * ZISK_PUBLICS {
            line.clear();
            match self.stderr.read_line(&mut line) {
                Ok(0) => return Err(self.exit_error(io::ErrorKind::UnexpectedEof.into())),
                Ok(_) => {}
                Err(err) => return Err(self.exit_error(err)),
            }
            let line = line.trim_end();
            if let Some((_, rest)) = line.split_once(STEPS_FIELD) {
                steps = rest[..rest.find(',').unwrap()].parse().unwrap();
            } else if line.len() == 8
                && let Ok(word) = u32::from_str_radix(line, 16)
            {
                public_values.extend(word.to_le_bytes());
            } else {
                let message = line.split_once("] ").map_or(line, |(_, message)| message);
                log.push(message.to_string());
            }
        }
        Ok((steps, log, public_values))
    }

    /// The error of a service that stopped, with the rest of its stderr.
    fn exit_error(&mut self, err: io::Error) -> Error {
        self.broken = true;
        let mut stderr = String::new();
        let _ = self.stderr.read_to_string(&mut stderr);
        Error::AsmEmulatorFailed(err, stderr)
    }
}

/// Removes the shared memory and the semaphore names of `shm_prefix` from `/dev/shm`.
fn remove_shared_memory(shm_prefix: &str) {
    let prefix = format!("{shm_prefix}_");
    for entry in fs::read_dir("/dev/shm").into_iter().flatten().flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name
            .strip_prefix("sem.")
            .unwrap_or(&name)
            .starts_with(&prefix)
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

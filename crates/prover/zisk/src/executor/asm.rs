//! Execution on the ASM emulator that ZisK generates from the ROM in Fast mode.

use std::{
    array,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::fs::{FileExt, symlink},
    path::{Path, PathBuf},
    process::{self, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::anyhow;
use ere_prover_core::{CommonError, PublicValues};
use parking_lot::Mutex;
use zisk_common::ZISK_PUBLICS;
use zisk_core::{
    AsmGenerationMethod, DEFAULT_MAX_STEPS, INPUT_ADDR, RAM_ADDR, RAM_SIZE, ZiskRom, ZiskRom2Asm,
};
use zisk_rom_setup::{ensure_ziskclib, get_elf_data_hash, get_output_path, resolve_emulator_asm};

use crate::error::Error;

// Request and response types of `emulator-asm/src/constants.hpp`.
const TYPE_PING: u64 = 1;
const TYPE_PONG: u64 = 2;
const TYPE_FA_REQUEST: u64 = 13;
const TYPE_FA_RESPONSE: u64 = 14;
const TYPE_SD_REQUEST: u64 = 1000000;

/// Linker script command of the Makefile that reserves the addresses of the input, the ROM, the RAM
/// and the MT and MO trace.
const TRACE_RESERVATION: &str = ". = . + 0x890000000;";

/// Step limit of a Fast request. Fast mode does not count against it, but the service exits unless
/// it is a power of two.
const MAX_STEPS: u64 = 1 << 36;

/// Chunk size of a Fast request, with the same constraint as [`MAX_STEPS`].
const CHUNK_SIZE: u64 = 1 << 18;

/// Start of the stderr line that gives the error code of an emulation.
const ERROR_CODE_LINE: &str = "Emulation ended with error code ";

/// Part of the stderr metrics line before the number of steps.
const STEPS_FIELD: &str = ", steps = ";

/// Number of services started in this process, which makes each shared memory prefix unique.
static SERVICE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Runs a program on an ASM emulator service, started on the first execution.
pub(crate) struct AsmExecutor {
    elf_hash: String,
    service: Mutex<Option<Service>>,
}

impl AsmExecutor {
    pub(crate) fn new(elf: &[u8]) -> Self {
        Self {
            elf_hash: get_elf_data_hash(elf),
            service: Mutex::new(None),
        }
    }

    /// Runs the framed `stdin` on the program of `rom`, blocking until the service is free.
    pub(crate) fn execute(
        &self,
        rom: &ZiskRom,
        stdin: &[u8],
    ) -> Result<(PublicValues, Duration), Error> {
        let mut service = self.service.lock();
        service.take_if(|service| service.exited());
        if service.is_none() {
            *service = Some(Service::spawn(&build(rom, &self.elf_hash)?)?);
        }
        let result = service.as_mut().unwrap().execute(stdin);
        if let Err(Error::AsmEmulatorFailed(..)) = result {
            *service = None;
        }
        result
    }
}

/// Builds the Fast binary of the program into the ZisK cache, unless it is there.
fn build(rom: &ZiskRom, elf_hash: &str) -> Result<PathBuf, Error> {
    let cache_dir = get_output_path(&None).map_err(Error::BuildAsmEmulator)?;
    let binary_path = cache_dir.join(format!("{elf_hash}-ft.bin"));
    if binary_path.exists() {
        return Ok(binary_path);
    }

    let (emulator_asm_dir, source) = resolve_emulator_asm().map_err(Error::BuildAsmEmulator)?;
    ensure_ziskclib(&emulator_asm_dir, source).map_err(Error::BuildAsmEmulator)?;

    // Every build in the `emulator-asm` directory writes the same `build` directory, so this one
    // runs in a private copy of the directory layout that the Makefile uses. The binary moves into
    // the cache when complete, so no other process runs a partial one.
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

    // Fast mode maps nothing past the RAM, and an exec that reserves the trace addresses too fails
    // on hosts with less memory than that reservation.
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

/// An ASM emulator service in Fast mode, which runs one request at a time.
struct Service {
    pipes: Pipes,
    input: File,
    control_input: File,
    owner: Option<JoinHandle<()>>,
}

impl Service {
    fn spawn(binary_path: &Path) -> Result<Self, Error> {
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

        // The service gets `SIGKILL` when the thread that spawns it ends, so that thread lives as
        // long as the service.
        let (sender, receiver) = mpsc::channel();
        let owner = thread::spawn(move || {
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(err) => {
                    return sender
                        .send(Err(CommonError::command(&command, err)))
                        .unwrap();
                }
            };
            let pipes = Pipes {
                stdin: child.stdin.take().unwrap(),
                stdout: child.stdout.take().unwrap(),
                stderr: BufReader::new(child.stderr.take().unwrap()),
            };
            sender.send(Ok(pipes)).unwrap();
            let _ = child.wait();
        });
        let mut pipes = receiver.recv().unwrap()?;

        // The service creates its shared memory before it answers. Its mappings and the files kept
        // here hold the memory after the names go, so the memory goes with the processes on any
        // exit.
        let files = pipes
            .request([TYPE_PING, 0, 0, 0, 0], TYPE_PONG)
            .and_then(|_| {
                let open = |name| {
                    let path = format!("/dev/shm/{shm_prefix}_FT_{name}");
                    File::options().write(true).open(&path).map_err(|err| {
                        CommonError::write_file("ASM emulator shared memory", &path, err)
                    })
                };
                Ok((open("input")?, open("control_input")?))
            });
        remove_shared_memory(&shm_prefix);
        let (input, control_input) = files?;

        Ok(Self {
            pipes,
            input,
            control_input,
            owner: Some(owner),
        })
    }

    fn exited(&self) -> bool {
        self.owner.as_ref().unwrap().is_finished()
    }

    fn execute(&mut self, stdin: &[u8]) -> Result<(PublicValues, Duration), Error> {
        // The service resets its memory after each response, so the timing starts after that reset.
        self.pipes.request([TYPE_PING, 0, 0, 0, 0], TYPE_PONG)?;

        let write = |file: &File, data: &[u8], offset| {
            file.write_all_at(data, offset)
                .map_err(|err| CommonError::io("Write ASM emulator shared memory", err))
        };

        let start = Instant::now();
        write(&self.input, &[0; 8], 0)?;
        write(&self.input, stdin, 8)?;
        // The control input holds the precompile size, the exit flag, the input size and the reset
        // flag. The reset flag turns a read past the input into an error after one 5 s wait, not a
        // hang.
        let control_input = [0, 0, stdin.len() as u64, 1].map(u64::to_le_bytes).concat();
        write(&self.control_input, &control_input, 0)?;
        let response = self.pipes.request(
            [TYPE_FA_REQUEST, MAX_STEPS, CHUNK_SIZE, 0, 0],
            TYPE_FA_RESPONSE,
        )?;
        let execution_duration = start.elapsed();

        let (steps, log, public_values) = self.pipes.read_output()?;
        if steps > DEFAULT_MAX_STEPS {
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
    fn drop(&mut self) {
        let _ = self
            .pipes
            .stdin
            .write_all(&[TYPE_SD_REQUEST, 0, 0, 0, 0].map(u64::to_le_bytes).concat());
        let _ = self.owner.take().unwrap().join();
    }
}

/// The stdio of a service, which carries requests and responses of five `u64` each and prints the
/// log and the output of each request to stderr before its response.
struct Pipes {
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: BufReader<ChildStderr>,
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
            return Err(Error::AsmEmulatorFailed(err, String::new()));
        }
        Ok(response)
    }

    /// Reads the number of steps, the log lines and the public values that the service prints to
    /// stderr before its response. Each hex line is a little-endian `u32` of the public values.
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

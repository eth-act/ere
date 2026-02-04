//! Local ZisK server management via `cargo-zisk` commands.

use crate::zkvm::{Error, sdk::dot_zisk_dir_path};
use ere_zkvm_interface::zkvm::CommonError;
use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    iter,
    net::{Ipv4Addr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use strum::{EnumIter, IntoEnumIterator};
use tempfile::tempdir;
use tracing::{error, info};
use wait_timeout::ChildExt;

pub const DEFAULT_START_SERVER_TIMEOUT_SEC: u64 = 120; // 2 mins
pub const DEFAULT_SHUTDOWN_SERVER_TIMEOUT_SEC: u64 = 30; // 30 secs
pub const DEFAULT_PROVE_TIMEOUT_SEC: u64 = 3600; // 1 hour

/// ZisK server status returned from `cargo-zisk prove-client status`.
#[derive(Debug)]
pub enum ZiskServerStatus {
    Idle,
    Working,
}

/// Options of `cargo-zisk` commands.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter)]
pub enum ZiskServerOption {
    Port,
    UnlockMappedMemory, // Should be set if locked memory is not enough
    MinimalMemory,
    // GPU options
    Preallocate, // Should be set only if GPU memory is enough
    SharedTables,
    MaxStreams,
    NumberThreadsWitness,
    MaxWitnessStored,
}

impl ZiskServerOption {
    /// The key of the env variable to read from.
    fn env_var_key(&self) -> &'static str {
        match self {
            Self::Port => "ZISK_PORT",
            Self::UnlockMappedMemory => "ZISK_UNLOCK_MAPPED_MEMORY",
            Self::MinimalMemory => "ZISK_MINIMAL_MEMORY",
            Self::Preallocate => "ZISK_PREALLOCATE",
            Self::SharedTables => "ZISK_SHARED_TABLES",
            Self::MaxStreams => "ZISK_MAX_STREAMS",
            Self::NumberThreadsWitness => "ZISK_NUMBER_THREADS_WITNESS",
            Self::MaxWitnessStored => "ZISK_MAX_WITNESS_STORED",
        }
    }

    /// Whether the option is a flag (false-by-default boolean option) or not.
    ///
    /// When we read the option from env variable, if the option is a flag,
    /// we only check if the env variable is set or not.
    fn is_flag(&self) -> bool {
        match self {
            Self::UnlockMappedMemory
            | Self::MinimalMemory
            | Self::Preallocate
            | Self::SharedTables => true,
            Self::Port | Self::MaxStreams | Self::NumberThreadsWitness | Self::MaxWitnessStored => {
                false
            }
        }
    }

    /// The option key to be appended to `cargo-zisk` command arguments.
    fn key(&self) -> &'static str {
        match self {
            Self::Port => "--port",
            Self::UnlockMappedMemory => "--unlock-mapped-memory",
            // NOTE: Use snake case for `prove-client` command
            // Issue for tracking: https://github.com/eth-act/ere/issues/151.
            Self::MinimalMemory => "--minimal_memory",
            Self::Preallocate => "--preallocate",
            Self::SharedTables => "--shared-tables",
            Self::MaxStreams => "--max-streams",
            Self::NumberThreadsWitness => "--number-threads-witness",
            Self::MaxWitnessStored => "--max-witness-stored",
        }
    }
}

/// Configurable options for `cargo-zisk server` and `cargo-zisk prove-client` commands.
#[derive(Clone)]
pub struct ZiskServerOptions(BTreeMap<ZiskServerOption, String>);

impl ZiskServerOptions {
    /// Read options from env variables.
    pub fn from_env() -> Self {
        Self(
            ZiskServerOption::iter()
                .flat_map(|option| env::var(option.env_var_key()).ok().map(|val| (option, val)))
                .collect(),
        )
    }

    /// Returns `cargo-zisk` command arguments by given options that have been
    /// set.
    fn args(
        &self,
        options: impl IntoIterator<Item = ZiskServerOption>,
    ) -> impl Iterator<Item = &str> {
        options
            .into_iter()
            .filter(|option| self.0.contains_key(option))
            .flat_map(|option| {
                iter::once(option.key())
                    .chain((!option.is_flag()).then(|| self.0[&option].as_str()))
            })
    }

    /// Returns `cargo-zisk server` command arguments.
    pub(crate) fn server_args(&self) -> impl Iterator<Item = &str> {
        self.args([
            ZiskServerOption::Port,
            ZiskServerOption::UnlockMappedMemory,
            ZiskServerOption::Preallocate,
            ZiskServerOption::SharedTables,
            ZiskServerOption::MaxStreams,
            ZiskServerOption::NumberThreadsWitness,
            ZiskServerOption::MaxWitnessStored,
        ])
    }

    /// Returns `cargo-zisk prove-client` command arguments.
    pub(crate) fn prove_client_args(&self) -> impl Iterator<Item = &str> {
        self.args([ZiskServerOption::Port])
    }

    /// Returns `cargo-zisk prove-client prove` command arguments.
    pub(crate) fn prove_args(&self) -> impl Iterator<Item = &str> {
        self.prove_client_args()
            .chain(self.args([ZiskServerOption::MinimalMemory]))
    }
}

/// Wrapper for ZisK server child process.
pub struct ZiskServer {
    options: ZiskServerOptions,
    elf_path: PathBuf,
    cuda: bool,
    child: Mutex<Option<Child>>,
}

impl Drop for ZiskServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl ZiskServer {
    /// Create a new ZisK server for the given ELF.
    ///
    /// The server process is lazily started on the first call to [`prove`](Self::prove).
    pub fn new(elf_path: &Path, cuda: bool, options: ZiskServerOptions) -> Self {
        Self {
            elf_path: elf_path.to_path_buf(),
            cuda,
            options,
            child: Mutex::new(None),
        }
    }

    /// Send prove request to server and wait for proof to be created.
    ///
    /// Returns the proof. Note that rom_digest validation should be performed by the caller.
    pub fn prove(&self, input: &[u8]) -> Result<Vec<u8>, Error> {
        self.ensure_ready()?;

        // Prefix that ZisK server will add to the file name of the proof.
        // We use constant because the file will be save to a temporary dir,
        // so there will be no conflict.
        const PREFIX: &str = "ere";

        let tempdir = tempdir().map_err(CommonError::tempdir)?;
        let input_path = tempdir.path().join("input");
        let output_path = tempdir.path().join("output");
        let proof_path = output_path.join(format!("{PREFIX}-vadcop_final_proof.bin"));

        fs::write(&input_path, input)
            .map_err(|err| CommonError::write_file("input", &input_path, err))?;

        // NOTE: Use snake case for `prove-client` command
        // Issue for tracking: https://github.com/eth-act/ere/issues/151.
        let mut cmd = Command::new("cargo-zisk");
        let output = cmd
            .args(["prove-client", "prove"])
            .arg("--input")
            .arg(input_path)
            .arg("--output_dir")
            .arg(&output_path)
            .args(["-p", PREFIX])
            .args(["--aggregation", "--verify_proofs"])
            .args(self.options.prove_args())
            .output()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !output.status.success() {
            return Err(CommonError::command_exit_non_zero(
                &cmd,
                output.status,
                Some(&output),
            ))?;
        }

        // ZisK server will finish the `prove` requested above then respond the
        // following `status`. So if the following `status` succeeds, the proof
        // should also be ready.
        self.status(prove_timeout()).map_err(|err| {
            if matches!(err, Error::TimeoutWaitingServerReady) {
                Error::TimeoutWaitingServerProving
            } else if err.to_string().contains("EOF") {
                Error::ServerCrashed
            } else {
                err
            }
        })?;

        let proof = fs::read(&proof_path)
            .map_err(|err| CommonError::read_file("proof", &proof_path, err))?;

        Ok(proof)
    }

    /// Ensure the server is running and responsive, restarting it if needed.
    fn ensure_ready(&self) -> Result<(), Error> {
        if self.child.lock().is_some() && self.status(start_server_timeout()).is_ok() {
            return Ok(());
        }

        const MAX_RETRY: usize = 3;
        let mut attempt = 0;

        loop {
            self.shutdown();
            match self.start() {
                Ok(()) => return Ok(()),
                Err(Error::TimeoutWaitingServerReady) if attempt < MAX_RETRY => {
                    error!("Timeout waiting server ready, restarting...");
                    attempt += 1;
                    continue;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Spawn the server process and wait until it's ready.
    fn start(&self) -> Result<(), Error> {
        info!("Starting ZisK server...");

        let (cargo_zisk, witness_lib_name) = if self.cuda {
            ("cargo-zisk-cuda", "libzisk_witness_cuda.so")
        } else {
            ("cargo-zisk", "libzisk_witness.so")
        };
        let witness_lib_path = dot_zisk_dir_path().join("bin").join(witness_lib_name);

        let mut cmd = Command::new(cargo_zisk);
        cmd.arg("server")
            .args(self.options.server_args())
            .arg("--elf")
            .arg(&self.elf_path)
            .arg("--witness-lib")
            .arg(witness_lib_path)
            .arg("--aggregation");

        let child = cmd.spawn().map_err(|err| CommonError::command(&cmd, err))?;

        {
            let mut guard = self.child.lock();
            *guard = Some(child);
        }

        self.wait_until_ready()?;

        Ok(())
    }

    /// Wait until the server status to be idle.
    pub fn wait_until_ready(&self) -> Result<(), Error> {
        const INTERVAL: Duration = Duration::from_secs(1);

        let timeout = start_server_timeout();

        info!("Waiting until server is ready...");

        let start = Instant::now();
        while !matches!(self.status(timeout), Ok(ZiskServerStatus::Idle)) {
            if start.elapsed() > timeout {
                return Err(Error::TimeoutWaitingServerReady);
            }
            thread::sleep(INTERVAL);
        }

        Ok(())
    }

    /// Gracefully shut down the server, falling back to force-kill on failure.
    fn shutdown(&self) {
        let mut guard = self.child.lock();
        let Some(mut child) = guard.take() else {
            return;
        };

        info!("Shutting down ZisK server");

        let mut cmd = Command::new("cargo-zisk");
        let result = cmd
            .args(["prove-client", "shutdown"])
            .args(self.options.prove_client_args())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(
                |mut child| match child.wait_timeout(shutdown_server_timeout())? {
                    Some(_) => child.wait_with_output(),
                    None => {
                        child.kill().ok();
                        Err(std::io::Error::other("shutdown command timed out"))
                    }
                },
            );

        if result.as_ref().is_ok_and(|output| output.status.success()) {
            info!("Shutdown ZisK server");
        } else {
            error!(
                "Failed to shutdown ZisK server: {}",
                result
                    .map(|output| String::from_utf8_lossy(&output.stderr).to_string())
                    .unwrap_or_else(|err| err.to_string())
            );
            error!("Shutdown server child process and asm services manually...");
            let _ = child.kill();
            shutdown_asm_service(23115);
            shutdown_asm_service(23116);
            shutdown_asm_service(23117);
            remove_shm_files();
        }
    }

    /// Get status of server.
    fn status(&self, timeout: Duration) -> Result<ZiskServerStatus, Error> {
        let mut cmd = Command::new("cargo-zisk");
        let mut child = cmd
            .args(["prove-client", "status"])
            .args(self.options.prove_client_args())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if child
            .wait_timeout(timeout)
            .map_err(|err| CommonError::command(&cmd, err))?
            .is_none()
        {
            // Timeout reached, kill the process
            child.kill().ok();
            return Err(Error::TimeoutWaitingServerReady);
        }

        let output = child
            .wait_with_output()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !output.status.success() {
            return Err(CommonError::command_exit_non_zero(
                &cmd,
                output.status,
                Some(&output),
            ))?;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("idle") {
            Ok(ZiskServerStatus::Idle)
        } else if stdout.contains("working") {
            Ok(ZiskServerStatus::Working)
        } else {
            Err(Error::UnknownServerStatus {
                stdout: stdout.to_string(),
            })
        }
    }
}

/// Send shutdown request to ZisK asm services.
fn shutdown_asm_service(port: u16) {
    // According to https://github.com/0xPolygonHermez/zisk/blob/v0.15.0/emulator-asm/asm-runner/src/asm_services/mod.rs#L34.
    const CMD_SHUTDOWN_REQUEST_ID: u64 = 1000000;
    if let Ok(mut stream) = TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
        let _ = stream.write_all(
            &[CMD_SHUTDOWN_REQUEST_ID, 0, 0, 0, 0]
                .into_iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>(),
        );
    }
}

/// Remove shared memory created by ZisK.
fn remove_shm_files() {
    let Ok(shm_dir) = fs::read_dir(Path::new("/dev/shm")) else {
        return;
    };

    for entry in shm_dir.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.starts_with("ZISK") || name.starts_with("sem"))
        {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Returns the server start timeout, configurable via `ZISK_START_SERVER_TIMEOUT_SEC`.
fn start_server_timeout() -> Duration {
    timeout(
        "ZISK_START_SERVER_TIMEOUT_SEC",
        DEFAULT_START_SERVER_TIMEOUT_SEC,
    )
}

/// Returns the server shutdown timeout, configurable via `ZISK_SHUTDOWN_SERVER_TIMEOUT_SEC`.
fn shutdown_server_timeout() -> Duration {
    timeout(
        "ZISK_SHUTDOWN_SERVER_TIMEOUT_SEC",
        DEFAULT_SHUTDOWN_SERVER_TIMEOUT_SEC,
    )
}

/// Returns the prove timeout, configurable via `ZISK_PROVE_TIMEOUT_SEC`.
fn prove_timeout() -> Duration {
    timeout("ZISK_PROVE_TIMEOUT_SEC", DEFAULT_PROVE_TIMEOUT_SEC)
}

/// Read a timeout from the given env variable key, falling back to `default`.
fn timeout(key: &str, default: u64) -> Duration {
    let sec = env::var(key)
        .ok()
        .and_then(|timeout| timeout.parse::<u64>().ok())
        .unwrap_or(default);
    Duration::from_secs(sec)
}

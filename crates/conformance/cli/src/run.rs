//! Runs ACT4 ELFs through the `ere-server` images published for an ere revision.

use std::{
    collections::BTreeSet,
    env, fmt, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, bail, ensure};
use clap::ValueEnum;
use ere_dockerized::{
    DockerizedzkVM, DockerizedzkVMConfig, Elf, Input, ProverResource, PublicValues,
    image::server_zkvm_image, zkVMKind,
};
use tracing::{debug, info, warn};

use crate::{
    elfs,
    provenance::Revision,
    record::{self, Outcome, Run, TestResult},
};

/// Registry that ere CI publishes images to.
const IMAGE_REGISTRY: &str = "ghcr.io/eth-act/ere";

#[derive(clap::Args)]
pub struct RunArgs {
    /// ere revision whose published images are tested (commit, tag or branch).
    #[arg(long)]
    rev: String,
    /// zkVMs to test.
    #[arg(long, value_delimiter = ',', default_values = ["openvm", "sp1", "zisk"])]
    zkvm: Vec<zkVMKind>,
    /// ACT4 suite to run.
    #[arg(long, value_enum, default_value_t = Suite::Act4Standard)]
    suite: Suite,
    /// Last stage to run; each stage runs only if the previous one passed.
    #[arg(long, value_enum, default_value_t = Mode::Execute)]
    mode: Mode,
    /// Prover resource [default: gpu when proving, cpu otherwise].
    #[arg(long, value_enum)]
    resource: Option<Resource>,
    /// Path to a zkevm-test-monitor checkout that generates the ELFs.
    #[arg(long, env = "ERE_MONITOR_PATH", required_unless_present = "elf_dir")]
    monitor_path: Option<PathBuf>,
    /// Use existing ELFs from `<ELF_DIR>/{native,target}` instead; the run is not recorded.
    #[arg(long)]
    elf_dir: Option<PathBuf>,
    /// Run only these tests, e.g. `I-add-00`; the run is not recorded.
    #[arg(long, value_delimiter = ',')]
    test: Vec<String>,
    /// Dashboard directory holding `config.json` and `data/history/`.
    #[arg(long, default_value = "dashboard")]
    dashboard: PathBuf,
    /// Directory for per-test details (outcomes, errors, timings).
    #[arg(long, default_value = "target/conformance")]
    details_dir: PathBuf,
    /// Do not append the run to the dashboard history.
    #[arg(long)]
    no_record: bool,
    /// Resolve the revision, image and ELFs, then stop.
    #[arg(long)]
    dry_run: bool,
    /// Seconds to wait for a server container to become healthy.
    #[arg(long, default_value_t = 120)]
    health_timeout: u64,
    /// Seconds allowed to start a server, including program key generation.
    #[arg(long, default_value_t = 900)]
    setup_timeout: u64,
    /// Seconds allowed to execute one test.
    #[arg(long, default_value_t = 600)]
    execute_timeout: u64,
    /// Seconds allowed to prove one test.
    #[arg(long, default_value_t = 3600)]
    prove_timeout: u64,
    /// Seconds allowed to verify one proof.
    #[arg(long, default_value_t = 600)]
    verify_timeout: u64,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Suite {
    /// Standard ISA target suite (72 tests).
    Act4Standard,
    /// Full ISA of each zkVM.
    Act4Full,
}

impl Suite {
    fn as_str(self) -> &'static str {
        match self {
            Self::Act4Standard => "act4-standard",
            Self::Act4Full => "act4-full",
        }
    }

    /// Subdirectory of the `./run elfs` output that holds this suite.
    fn elf_subdir(self) -> &'static str {
        match self {
            Self::Act4Standard => "target",
            Self::Act4Full => "native",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Mode {
    Execute,
    Prove,
    Verify,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Execute => "execute",
            Self::Prove => "prove",
            Self::Verify => "verify",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Resource {
    Cpu,
    Gpu,
}

pub fn run(args: RunArgs) -> anyhow::Result<()> {
    ensure!(
        env::var_os("ERE_FORCE_REBUILD_DOCKER_IMAGE").is_none(),
        "unset ERE_FORCE_REBUILD_DOCKER_IMAGE: conformance runs test published images only"
    );

    let revision = Revision::resolve(&args.rev)?;
    let resource = match args.resource.unwrap_or(match args.mode {
        Mode::Execute => Resource::Cpu,
        Mode::Prove | Mode::Verify => Resource::Gpu,
    }) {
        Resource::Cpu => ProverResource::Cpu,
        Resource::Gpu => ProverResource::Gpu,
    };
    let record = !args.no_record && !args.dry_run && args.elf_dir.is_none() && args.test.is_empty();

    // SAFETY: No other threads exist yet. `ere-dockerized` reads these when it
    // names, pulls and starts server containers.
    unsafe {
        if env::var_os("ERE_IMAGE_REGISTRY").is_none() {
            env::set_var("ERE_IMAGE_REGISTRY", IMAGE_REGISTRY);
        }
        env::set_var("ERE_IMAGE_TAG", revision.image_tag());
        // ZisK downloads its proving key on demand; keep it across containers.
        if env::var_os("ERE_ZISK_PROVING_KEY_VOLUME").is_none() {
            env::set_var("ERE_ZISK_PROVING_KEY_VOLUME", "ere-zisk-proving-key");
        }
    }

    info!(
        "ere {} ({}), suite {}, mode {}, resource {}",
        revision.ere_version,
        revision.image_tag(),
        args.suite.as_str(),
        args.mode.as_str(),
        resource.kind(),
    );

    let timeouts = Timeouts {
        health: Duration::from_secs(args.health_timeout),
        setup: Duration::from_secs(args.setup_timeout),
        execute: Duration::from_secs(args.execute_timeout),
        prove: Duration::from_secs(args.prove_timeout),
        verify: Duration::from_secs(args.verify_timeout),
    };

    for &zkvm_kind in &args.zkvm {
        let zkvm = zkvm_kind.as_str();
        let sdk_version = revision.sdk_version(zkvm_kind)?;
        let image = pull_image(zkvm_kind, resource.is_gpu())?;

        let (elf_root, act4) = match (&args.elf_dir, &args.monitor_path) {
            (Some(elf_dir), _) => (elf_dir.clone(), None),
            (None, Some(monitor_path)) => {
                let elfs = elfs::generate(monitor_path, zkvm_kind, false)?;
                (elfs.elf_dir.clone(), Some(elfs))
            }
            (None, None) => unreachable!("clap requires --monitor-path or --elf-dir"),
        };
        let elf_paths = find_elfs(&elf_root.join(args.suite.elf_subdir()), &args.test)?;
        info!(
            "{zkvm}: {} ELFs, SDK {sdk_version}, image {image}",
            elf_paths.len()
        );

        if args.dry_run {
            elf_paths
                .iter()
                .for_each(|path| println!("{}", path.display()));
            continue;
        }

        let isa = record::isa(&args.dashboard, zkvm);
        let template = Run {
            date: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            ere_rev: revision.image_tag().to_string(),
            ere_version: revision.ere_version.clone(),
            ere_image: image,
            sdk_version,
            act4_commit: act4
                .as_ref()
                .map_or("unknown", |elfs| &elfs.act4_commit)
                .to_string(),
            act4_version: act4
                .as_ref()
                .map_or("unknown", |elfs| &elfs.act4_version)
                .to_string(),
            isa: if record {
                isa?
            } else {
                isa.unwrap_or_else(|_| "unknown".to_string())
            },
            mode: args.mode.as_str().to_string(),
            resource: resource.kind().to_string(),
            total: 0,
            passed: Vec::new(),
            failed: Vec::new(),
            prove_failed: Vec::new(),
            verify_failed: Vec::new(),
            has_proving: args.mode >= Mode::Prove,
        };
        let details = args.details_dir.join(format!(
            "{zkvm}-{}-{}-{}-{}.json",
            args.suite.as_str(),
            revision.image_tag(),
            args.mode.as_str(),
            template.date.replace(['-', ':'], ""),
        ));
        info!("details: {}", details.display());

        let started = Instant::now();
        let total = elf_paths.len();
        let mut results = Vec::with_capacity(total);
        for (idx, path) in elf_paths.iter().enumerate() {
            let result = run_elf(zkvm_kind, path, &resource, args.mode, timeouts);
            let status = match result.outcome {
                Outcome::Passed => "PASS",
                Outcome::Failed => "FAIL",
                Outcome::ProveFailed => "PROVE-FAIL",
                Outcome::VerifyFailed => "VERIFY-FAIL",
            };
            info!(
                "[{:>3}/{total}] {status} {} ({})",
                idx + 1,
                result.name,
                timings(&result)
            );
            if let Some(error) = &result.error {
                warn!("  {error}");
            }
            results.push(result);
            // Save after every test, so an interrupted run keeps its results.
            record::write_details(&details, &template.clone().with_results(&results), &results)?;
        }
        let run = template.with_results(&results);

        info!(
            "{zkvm}: {}/{} passed ({} failed, {} prove failed, {} verify failed) in {:.0?}",
            run.passed.len(),
            run.total,
            run.failed.len(),
            run.prove_failed.len(),
            run.verify_failed.len(),
            started.elapsed(),
        );

        if record {
            let history = record::append(&args.dashboard, zkvm, args.suite.as_str(), &run)?;
            info!("recorded: {}", history.display());
        } else {
            info!("not recorded (--no-record, --dry-run, --elf-dir or --test)");
        }
    }

    Ok(())
}

/// Pulls the server image of the revision under test, so `ere-dockerized`
/// never falls back to building it, and returns its content digest.
fn pull_image(zkvm_kind: zkVMKind, gpu: bool) -> anyhow::Result<String> {
    let image = server_zkvm_image(zkvm_kind, gpu);
    let status = Command::new("docker")
        .args(["pull", "--quiet", &image])
        .stdout(Stdio::null())
        .status()
        .context("failed to run docker")?;
    ensure!(
        status.success(),
        "no published image {image}; ere publishes images for commits on master and release/*"
    );

    let output = Command::new("docker")
        .args([
            "image",
            "inspect",
            "--format",
            "{{index .RepoDigests 0}}",
            &image,
        ])
        .output()
        .context("failed to run docker")?;
    ensure!(output.status.success(), "failed to inspect {image}");
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

/// Returns the `*.elf` files under `dir`, sorted by test name, optionally
/// restricted to `tests`.
fn find_elfs(dir: &Path, tests: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, elfs: &mut Vec<PathBuf>) -> anyhow::Result<()> {
        for entry in
            fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                walk(&path, elfs)?;
            } else if path.extension().is_some_and(|ext| ext == "elf") {
                elfs.push(path);
            }
        }
        Ok(())
    }

    let mut elfs = Vec::new();
    walk(dir, &mut elfs)?;
    elfs.sort_by_key(|path| test_name(path));

    if !tests.is_empty() {
        elfs.retain(|path| tests.contains(&test_name(path)));
        let found = BTreeSet::from_iter(elfs.iter().map(|path| test_name(path)));
        let missing = Vec::from_iter(tests.iter().filter(|test| !found.contains(*test)));
        ensure!(
            missing.is_empty(),
            "tests not found in {}: {missing:?}",
            dir.display()
        );
    }
    if elfs.is_empty() {
        bail!("no ELFs found in {}", dir.display());
    }
    Ok(elfs)
}

fn test_name(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Test stages, in order.
#[derive(Clone, Copy, Debug)]
enum Stage {
    Setup,
    Execute,
    Prove,
    Verify,
}

impl Stage {
    /// Outcome of a test that does not get past this stage.
    fn failure(self) -> Outcome {
        match self {
            Self::Setup | Self::Execute => Outcome::Failed,
            Self::Prove => Outcome::ProveFailed,
            Self::Verify => Outcome::VerifyFailed,
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Setup => "setup",
            Self::Execute => "execute",
            Self::Prove => "prove",
            Self::Verify => "verify",
        })
    }
}

/// Time limits per stage. `ere-dockerized` enforces the RPC ones itself; the
/// watchdog in [`run_elf`] enforces all of them, with some slack.
#[derive(Clone, Copy, Debug)]
struct Timeouts {
    health: Duration,
    setup: Duration,
    execute: Duration,
    prove: Duration,
    verify: Duration,
}

impl Timeouts {
    const SLACK: Duration = Duration::from_secs(60);

    fn config(&self) -> DockerizedzkVMConfig {
        DockerizedzkVMConfig {
            execute_timeout: Some(self.execute),
            prove_timeout: Some(self.prove),
            verify_timeout: Some(self.verify),
            health_timeout: self.health,
        }
    }

    fn watchdog(&self, stage: Stage) -> Duration {
        Self::SLACK
            + match stage {
                Stage::Setup => self.setup,
                Stage::Execute => self.execute,
                Stage::Prove => self.prove,
                Stage::Verify => self.verify,
            }
    }
}

/// Where a test thread is, shared with the watchdog in [`run_elf`].
struct Progress {
    stage: Stage,
    deadline: Instant,
    abandoned: bool,
}

/// Runs one ELF up to `mode`, in a fresh server container.
///
/// The test runs on its own thread. If a stage outlives its deadline, the
/// test is recorded as failed at that stage and abandoned: the thread is left
/// behind, stops at the next stage boundary, and the next test's container
/// replaces its container, which has the same name.
fn run_elf(
    zkvm_kind: zkVMKind,
    path: &Path,
    resource: &ProverResource,
    mode: Mode,
    timeouts: Timeouts,
) -> TestResult {
    let name = test_name(path);
    let progress = Arc::new(Mutex::new(Progress {
        stage: Stage::Setup,
        deadline: Instant::now() + timeouts.watchdog(Stage::Setup),
        abandoned: false,
    }));

    let (tx, rx) = mpsc::channel();
    let spawned = {
        let (path, resource, progress) = (path.to_path_buf(), resource.clone(), progress.clone());
        thread::Builder::new()
            .name(format!("test {name}"))
            .spawn(move || {
                let mut result = TestResult::new(test_name(&path));
                if let Err((outcome, error)) = run_stages(
                    zkvm_kind,
                    &path,
                    &resource,
                    mode,
                    timeouts,
                    &progress,
                    &mut result,
                ) {
                    result.outcome = outcome;
                    result.error = Some(error);
                }
                let _ = tx.send(result);
            })
    };
    if let Err(err) = spawned {
        let mut result = TestResult::new(name);
        result.outcome = Outcome::Failed;
        result.error = Some(format!("failed to spawn test thread: {err}"));
        return result;
    }

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(result) => return result,
            Err(RecvTimeoutError::Disconnected) => {
                let mut result = TestResult::new(name);
                result.outcome = progress.lock().unwrap().stage.failure();
                result.error = Some("test thread panicked".to_string());
                return result;
            }
            Err(RecvTimeoutError::Timeout) => {
                let mut progress = progress.lock().unwrap();
                if Instant::now() >= progress.deadline {
                    progress.abandoned = true;
                    let mut result = TestResult::new(name);
                    result.outcome = progress.stage.failure();
                    result.error = Some(format!(
                        "abandoned: {} stage exceeded {:?}",
                        progress.stage,
                        timeouts.watchdog(progress.stage)
                    ));
                    return result;
                }
            }
        }
    }
}

/// Runs the stages up to `mode` in order, stopping at the first stage that
/// does not pass.
fn run_stages(
    zkvm_kind: zkVMKind,
    path: &Path,
    resource: &ProverResource,
    mode: Mode,
    timeouts: Timeouts,
    progress: &Mutex<Progress>,
    result: &mut TestResult,
) -> Result<(), (Outcome, String)> {
    let enter = |stage: Stage| {
        let mut progress = progress.lock().unwrap();
        if progress.abandoned {
            return Err((stage.failure(), "abandoned".to_string()));
        }
        progress.stage = stage;
        progress.deadline = Instant::now() + timeouts.watchdog(stage);
        debug!("{}: {stage}", result.name);
        Ok(())
    };

    enter(Stage::Setup)?;
    let elf =
        fs::read(path).map_err(|err| (Outcome::Failed, format!("failed to read ELF: {err}")))?;
    let (zkvm, secs) =
        timed(|| DockerizedzkVM::new(zkvm_kind, Elf(elf), resource.clone(), timeouts.config()));
    result.setup_secs = secs;
    let zkvm = zkvm.map_err(|err| (Outcome::Failed, format!("server did not start: {err:#}")))?;

    enter(Stage::Execute)?;
    let input = Input::new();
    let (executed, secs) = timed(|| zkvm.execute(&input));
    result.execute_secs = Some(secs);
    verdict(zkvm_kind, executed.map(|(public_values, _)| public_values))
        .map_err(|error| (Outcome::Failed, error))?;
    if mode == Mode::Execute {
        return Ok(());
    }

    enter(Stage::Prove)?;
    let (proved, secs) = timed(|| zkvm.prove(&input));
    result.prove_secs = Some(secs);
    let (public_values, proof, _) =
        proved.map_err(|err| (Outcome::ProveFailed, format!("{err:#}")))?;
    verdict(zkvm_kind, Ok(public_values)).map_err(|error| (Outcome::ProveFailed, error))?;
    if mode == Mode::Prove {
        return Ok(());
    }

    enter(Stage::Verify)?;
    let (verified, secs) = timed(|| zkvm.verify(&proof));
    result.verify_secs = Some(secs);
    verdict(zkvm_kind, verified).map_err(|error| (Outcome::VerifyFailed, error))
}

/// Decides whether a stage passed.
///
/// SP1 and OpenVM report a non-zero ACT4 exit code as an error. ZisK ignores
/// the exit code, so its ACT4 halt macros also store `PASS` or `FAIL` to public
/// output 0 (see zkevm-test-monitor `act4-configs/zisk/*/rvmodel_macros.h`).
fn verdict(zkvm_kind: zkVMKind, result: anyhow::Result<PublicValues>) -> Result<(), String> {
    let public_values = result.map_err(|err| format!("{err:#}"))?;
    match zkvm_kind {
        zkVMKind::Zisk if !public_values.starts_with(b"PASS") => {
            let marker = &public_values[..public_values.len().min(4)];
            let shown = match marker {
                b"FAIL" => "FAIL".to_string(),
                _ => format!(
                    "0x{}",
                    String::from_iter(marker.iter().map(|b| format!("{b:02x}")))
                ),
            };
            Err(format!(
                "ZisK test reported {shown} in public output 0, not PASS"
            ))
        }
        _ => Ok(()),
    }
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let started = Instant::now();
    let value = f();
    (value, started.elapsed().as_secs_f64())
}

fn timings(result: &TestResult) -> String {
    let stages = [
        ("setup", Some(result.setup_secs)),
        ("execute", result.execute_secs),
        ("prove", result.prove_secs),
        ("verify", result.verify_secs),
    ];
    Vec::from_iter(
        stages
            .into_iter()
            .filter_map(|(stage, secs)| secs.map(|secs| format!("{stage} {secs:.1}s"))),
    )
    .join(", ")
}

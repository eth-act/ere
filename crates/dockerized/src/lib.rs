//! # Ere Dockerized
//!
//! A Docker-based wrapper for other zkVM crates `ere-{zkvm}`.
//!
//! This crate provides a unified interface to dockerize the `Compiler` and
//! `zkVM` implementation of other zkVM crates `ere-{zkvm}`, it requires only
//! `docker` to be installed, but no zkVM specific SDK.
//!
//! ## Docker image building
//!
//! It builds 4 Docker images in sequence if they don't exist:
//! 1. `ere-base:{version}` - Base image with common dependencies
//! 2. `ere-base-{zkvm}:{version}` - zkVM-specific base image with the zkVM SDK
//! 3. `ere-compiler-{zkvm}:{version}` - Compiler image with the `ere-compiler`
//!    binary built with the selected zkVM feature
//! 4. `ere-server-{zkvm}:{version}` - Server image with the `ere-server` binary
//!    built with the selected zkVM feature
//!
//! When [`ProverResourceType::Gpu`] is selected, the image with GPU support
//! will be built and tagged with specific suffix.
//!
//! To force rebuild all images, set the environment variable
//! `ERE_FORCE_REBUILD_DOCKER_IMAGE` to non-empty value.
//!
//! ## Example
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use ere_dockerized::{EreDockerizedCompiler, EreDockerizedzkVM, ErezkVM};
//! use zkvm_interface::{Compiler, Input, ProofKind, ProverResourceType, zkVM};
//! use std::path::Path;
//!
//! // The zkVM we plan to use
//! let zkvm = ErezkVM::SP1;
//!
//! // Compile a guest program
//! let compiler = EreDockerizedCompiler::new(zkvm, "mounting/directory")?;
//! let guest_path = Path::new("relative/path/to/guest/program");
//! let program = compiler.compile(&guest_path)?;
//!
//! // Create zkVM instance
//! let resource = ProverResourceType::Cpu;
//! let zkvm = EreDockerizedzkVM::new(zkvm, program, resource)?;
//!
//! // Prepare inputs
//! let mut inputs = Input::new();
//! inputs.write(42u32);
//! inputs.write(100u16);
//!
//! // Execute program
//! let (public_values, execution_report) = zkvm.execute(&inputs)?;
//! println!("Execution cycles: {}", execution_report.total_num_cycles);
//!
//! // Generate proof
//! let (public_values, proof, proving_report) = zkvm.prove(&inputs, ProofKind::Compressed)?;
//! println!("Proof generated in: {:?}", proving_report.proving_time);
//!
//! // Verify proof
//! let public_values = zkvm.verify(&proof)?;
//! println!("Proof verified successfully!");
//! # Ok(())
//! # }
//! ```

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    cuda::cuda_arch,
    docker::{DockerBuildCmd, DockerRunCmd, docker_image_exists, stop_docker_container},
    error::{CommonError, CompileError, DockerizedError, ExecuteError, ProveError, VerifyError},
};
use ere_server::client::{Url, zkVMClient};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    env,
    fmt::{self, Display, Formatter},
    fs,
    io::Read,
    iter,
    path::{Path, PathBuf},
    str::FromStr,
};
use tempfile::TempDir;
use tracing::error;
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/crate_version.rs"));
include!(concat!(env!("OUT_DIR"), "/zkvm_sdk_version_impl.rs"));

pub mod cuda;
pub mod docker;
pub mod error;
pub mod input;
pub mod output;

/// Offset of port used for `ere-server` for [`ErezkVM`]s.
const ERE_SERVER_PORT_OFFSET: u16 = 4174;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErezkVM {
    Jolt,
    Miden,
    Nexus,
    OpenVM,
    Pico,
    Risc0,
    SP1,
    Ziren,
    Zisk,
}

impl ErezkVM {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Jolt => "jolt",
            Self::Miden => "miden",
            Self::Nexus => "nexus",
            Self::OpenVM => "openvm",
            Self::Pico => "pico",
            Self::Risc0 => "risc0",
            Self::SP1 => "sp1",
            Self::Ziren => "ziren",
            Self::Zisk => "zisk",
        }
    }

    /// Tag of images in format of `{version}{suffix}`.
    fn image_tag(&self, version: &str, gpu: bool) -> String {
        let suffix = match (gpu, self) {
            // Only the following zkVMs requires CUDA setup in the base image
            // when GPU support is required.
            (true, Self::OpenVM | Self::Risc0 | Self::Zisk) => "-cuda",
            _ => "",
        };
        format!("{version}{suffix}")
    }

    pub fn base_image(&self, version: &str, gpu: bool) -> String {
        format!("ere-base:{}", self.image_tag(version, gpu))
    }

    pub fn base_zkvm_image(&self, version: &str, gpu: bool) -> String {
        format!("ere-base-{self}:{}", self.image_tag(version, gpu))
    }

    pub fn compiler_zkvm_image(&self, version: &str) -> String {
        format!("ere-compiler-{self}:{}", self.image_tag(version, false))
    }

    pub fn server_zkvm_image(&self, version: &str, gpu: bool) -> String {
        format!("ere-server-{self}:{}", self.image_tag(version, gpu))
    }

    /// This method builds 4 Docker images in sequence:
    /// 1. `ere-base:{version}`: Base image with common dependencies
    /// 2. `ere-base-{zkvm}:{version}`: zkVM-specific base image with the zkVM SDK
    /// 3. `ere-compiler-{zkvm}:{version}` - Compiler image with the `ere-compiler`
    ///    binary built with the selected zkVM feature
    /// 4. `ere-server-{zkvm}:{version}` - Server image with the `ere-server` binary
    ///    built with the selected zkVM feature
    ///
    /// When [`ProverResourceType::Gpu`] is selected, the image with GPU support
    /// will be built and tagged with specific suffix.
    ///
    /// Images are cached and only rebuilt if they don't exist or if the
    /// `ERE_FORCE_REBUILD_DOCKER_IMAGE` environment variable is set.
    pub fn build_docker_image(&self, gpu: bool) -> Result<(), CommonError> {
        let workspace_dir = workspace_dir();
        let docker_dir = workspace_dir.join("docker");

        let force_rebuild = env::var_os("ERE_FORCE_REBUILD_DOCKER_IMAGE").is_some();

        // Build `ere-base`
        if force_rebuild || !docker_image_exists(self.base_image(CRATE_VERSION, gpu))? {
            let mut cmd = DockerBuildCmd::new()
                .file(docker_dir.join("base").join("Dockerfile.base"))
                .tag(self.base_image(CRATE_VERSION, gpu))
                .tag(self.base_image("latest", gpu));

            if gpu {
                cmd = cmd.build_arg("CUDA", "1");
            }

            cmd.exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        // Build `ere-base-{zkvm}`
        if force_rebuild || !docker_image_exists(self.base_zkvm_image(CRATE_VERSION, gpu))? {
            let mut cmd = DockerBuildCmd::new()
                .file(docker_dir.join(self.as_str()).join("Dockerfile.base"))
                .tag(self.base_zkvm_image(CRATE_VERSION, gpu))
                .tag(self.base_zkvm_image("latest", gpu))
                .build_arg("BASE_IMAGE", self.base_image(CRATE_VERSION, gpu));

            if gpu {
                cmd = cmd.build_arg("CUDA", "1");

                let cuda_arch = cuda_arch();
                match self {
                    ErezkVM::OpenVM => {
                        if let Some(cuda_arch) = cuda_arch {
                            // OpenVM takes only the numeric part.
                            cmd = cmd.build_arg("CUDA_ARCH", cuda_arch.replace("sm_", ""))
                        }
                    }
                    ErezkVM::Risc0 | ErezkVM::Zisk => {
                        if let Some(cuda_arch) = cuda_arch {
                            cmd = cmd.build_arg("CUDA_ARCH", cuda_arch)
                        }
                    }
                    _ => {}
                }
            }

            cmd.exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        // Build `ere-compiler-{zkvm}`
        if force_rebuild || !docker_image_exists(self.compiler_zkvm_image(CRATE_VERSION))? {
            DockerBuildCmd::new()
                .file(docker_dir.join(self.as_str()).join("Dockerfile.compiler"))
                .tag(self.compiler_zkvm_image(CRATE_VERSION))
                .tag(self.compiler_zkvm_image("latest"))
                .build_arg("BASE_ZKVM_IMAGE", self.base_zkvm_image(CRATE_VERSION, gpu))
                .exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        // Build `ere-server-{zkvm}`
        if force_rebuild || !docker_image_exists(self.server_zkvm_image(CRATE_VERSION, gpu))? {
            let mut cmd = DockerBuildCmd::new()
                .file(docker_dir.join(self.as_str()).join("Dockerfile.server"))
                .tag(self.server_zkvm_image(CRATE_VERSION, gpu))
                .tag(self.server_zkvm_image("latest", gpu))
                .build_arg("BASE_ZKVM_IMAGE", self.base_zkvm_image(CRATE_VERSION, gpu));

            if gpu {
                cmd = cmd.build_arg("CUDA", "1");
            }

            cmd.exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        Ok(())
    }

    fn server_port(&self) -> u16 {
        ERE_SERVER_PORT_OFFSET + *self as u16
    }

    fn spawn_server(
        &self,
        program: &SerializedProgram,
        resource: &ProverResourceType,
    ) -> Result<ServerContainer, CommonError> {
        let port = self.server_port().to_string();
        let name = format!("ere-server-{self}-{port}");
        let gpu = matches!(resource, ProverResourceType::Gpu);
        let mut cmd = DockerRunCmd::new(self.server_zkvm_image(CRATE_VERSION, gpu))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .publish(&port, &port)
            .name(&name);

        // zkVM specific options
        cmd = match self {
            ErezkVM::Risc0 => cmd
                .inherit_env("RISC0_SEGMENT_PO2")
                .inherit_env("RISC0_KECCAK_PO2"),
            // ZisK uses shared memory to exchange data between processes, it
            // requires at least 8G shared memory, here we set 16G for safety.
            ErezkVM::Zisk => cmd
                .option("shm-size", "16G")
                .option("ulimit", "memlock=-1:-1")
                .inherit_env("ZISK_PORT")
                .inherit_env("ZISK_CHUNK_SIZE_BITS")
                .inherit_env("ZISK_UNLOCK_MAPPED_MEMORY")
                .inherit_env("ZISK_MINIMAL_MEMORY")
                .inherit_env("ZISK_PREALLOCATE")
                .inherit_env("ZISK_SHARED_TABLES")
                .inherit_env("ZISK_MAX_STREAMS")
                .inherit_env("ZISK_NUMBER_THREADS_WITNESS")
                .inherit_env("ZISK_MAX_WITNESS_STORED"),
            _ => cmd,
        };

        // zkVM specific options when using GPU
        if gpu {
            cmd = match self {
                ErezkVM::OpenVM => cmd.gpus("all"),
                // SP1 runs docker command to spin up the server to do GPU
                // proving, to give the client access to the prover service, we
                // need to use the host networking driver.
                ErezkVM::SP1 => cmd.mount_docker_socket().network("host"),
                ErezkVM::Risc0 => cmd.gpus("all").inherit_env("RISC0_DEFAULT_PROVER_NUM_GPUS"),
                ErezkVM::Zisk => cmd.gpus("all"),
                _ => cmd,
            }
        }

        let tempdir = TempDir::new()
            .map_err(|err| CommonError::io(err, "Failed to create temporary directory"))?;

        // zkVM specific options needed for proving Groth16 proof.
        cmd = match self {
            // Risc0 and SP1 runs docker command to prove Groth16 proof, and
            // they pass the input by mounting temporary directory. Here we
            // create a temporary directory and mount it on the top level, so
            // the volume could be shared, and override `TMPDIR` so we don't
            // need to mount the whole `/tmp`.
            ErezkVM::Risc0 => cmd
                .mount_docker_socket()
                .env("TMPDIR", tempdir.path().to_string_lossy())
                .volume(tempdir.path(), tempdir.path()),
            ErezkVM::SP1 => {
                let groth16_circuit_path = home_dir().join(".sp1").join("circuits").join("groth16");
                cmd.mount_docker_socket()
                    .env(
                        "SP1_GROTH16_CIRCUIT_PATH",
                        groth16_circuit_path.to_string_lossy(),
                    )
                    .env("TMPDIR", tempdir.path().to_string_lossy())
                    .volume(tempdir.path(), tempdir.path())
                    .volume(&groth16_circuit_path, &groth16_circuit_path)
            }
            _ => cmd,
        };

        let args = iter::empty()
            .chain(["--port", &port])
            .chain(resource.to_args());
        cmd.spawn(args, &program.0)
            .map_err(CommonError::DockerRunCmd)?;

        Ok(ServerContainer { name, tempdir })
    }
}

impl FromStr for ErezkVM {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "jolt" => Self::Jolt,
            "miden" => Self::Miden,
            "nexus" => Self::Nexus,
            "openvm" => Self::OpenVM,
            "pico" => Self::Pico,
            "risc0" => Self::Risc0,
            "sp1" => Self::SP1,
            "ziren" => Self::Ziren,
            "zisk" => Self::Zisk,
            _ => return Err(format!("Unsupported zkvm {s}")),
        })
    }
}

impl Display for ErezkVM {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct EreDockerizedCompiler {
    zkvm: ErezkVM,
    mount_directory: PathBuf,
}

impl EreDockerizedCompiler {
    pub fn new(zkvm: ErezkVM, mount_directory: impl AsRef<Path>) -> Result<Self, CommonError> {
        zkvm.build_docker_image(false)?;
        Ok(Self {
            zkvm,
            mount_directory: mount_directory.as_ref().to_path_buf(),
        })
    }

    pub fn zkvm(&self) -> ErezkVM {
        self.zkvm
    }
}

/// Wrapper for serialized program.
#[derive(Clone, Serialize, Deserialize)]
pub struct SerializedProgram(Vec<u8>);

impl Compiler for EreDockerizedCompiler {
    type Error = CompileError;
    type Program = SerializedProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let guest_relative_path = guest_directory
            .strip_prefix(&self.mount_directory)
            .map_err(|_| CompileError::GuestNotInMountingDirecty {
                mounting_directory: self.mount_directory.to_path_buf(),
                guest_directory: guest_directory.to_path_buf(),
            })?;
        let guest_path_in_docker = PathBuf::from("/guest").join(guest_relative_path);

        let tempdir = TempDir::new()
            .map_err(|err| CommonError::io(err, "Failed to create temporary directory"))?;

        let mut cmd = DockerRunCmd::new(self.zkvm.compiler_zkvm_image(CRATE_VERSION))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .inherit_env("ERE_RUST_TOOLCHAIN")
            .volume(&self.mount_directory, "/guest")
            .volume(tempdir.path(), "/output");

        cmd = match self.zkvm {
            // OpenVM allows to select Rust toolchain for guest compilation.
            ErezkVM::OpenVM => cmd.inherit_env("OPENVM_RUST_TOOLCHAIN"),
            _ => cmd,
        };

        cmd.exec([
            "--guest-path",
            guest_path_in_docker.to_string_lossy().as_ref(),
            "--output-path",
            "/output/program",
        ])
        .map_err(CommonError::DockerRunCmd)?;

        let program_path = tempdir.path().join("program");
        let program = fs::read(&program_path).map_err(|err| {
            CommonError::io(
                err,
                format!(
                    "Failed to read compiled program at {}",
                    program_path.display()
                ),
            )
        })?;
        Ok(SerializedProgram(program))
    }
}

struct ServerContainer {
    name: String,
    #[allow(dead_code)]
    tempdir: TempDir,
}

impl Drop for ServerContainer {
    fn drop(&mut self) {
        if let Err(err) = stop_docker_container(&self.name) {
            error!("{err}");
        }
    }
}

pub struct EreDockerizedzkVM {
    zkvm: ErezkVM,
    program: SerializedProgram,
    resource: ProverResourceType,
    #[allow(dead_code)]
    server_container: ServerContainer,
    client: zkVMClient,
}

impl EreDockerizedzkVM {
    pub fn new(
        zkvm: ErezkVM,
        program: SerializedProgram,
        resource: ProverResourceType,
    ) -> Result<Self, zkVMError> {
        zkvm.build_docker_image(matches!(resource, ProverResourceType::Gpu))?;

        let server_container = zkvm.spawn_server(&program, &resource)?;

        let url = Url::parse(&format!("http://127.0.0.1:{}", zkvm.server_port())).unwrap();
        let client = block_on(zkVMClient::new(url)).map_err(zkVMError::other)?;

        Ok(Self {
            zkvm,
            program,
            resource,
            server_container,
            client,
        })
    }

    pub fn zkvm(&self) -> ErezkVM {
        self.zkvm
    }

    pub fn program(&self) -> &SerializedProgram {
        &self.program
    }

    pub fn resource(&self) -> &ProverResourceType {
        &self.resource
    }
}

impl zkVM for EreDockerizedzkVM {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let serialized_input = self
            .zkvm
            .serialize_inputs(inputs)
            .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        let (public_values, report) = block_on(self.client.execute(serialized_input))
            .map_err(|err| DockerizedError::Execute(ExecuteError::Client(err)))?;

        Ok((public_values, report))
    }

    fn prove(
        &self,
        inputs: &Input,
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        let serialized_input = self
            .zkvm
            .serialize_inputs(inputs)
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;

        let (public_values, proof, report) =
            block_on(self.client.prove(serialized_input, proof_kind))
                .map_err(|err| DockerizedError::Prove(ProveError::Client(err)))?;

        Ok((public_values, proof, report))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let public_values = block_on(self.client.verify(proof))
            .map_err(|err| DockerizedError::Verify(VerifyError::Client(err)))?;

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        self.zkvm.as_str()
    }

    fn sdk_version(&self) -> &'static str {
        self.zkvm.sdk_version()
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, reader: R) -> Result<T, zkVMError> {
        self.zkvm.deserialize_from(reader)
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tokio::runtime::Runtime::new().unwrap().block_on(future),
    }
}

fn workspace_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.canonicalize().unwrap()
}

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
}

#[cfg(test)]
mod test {
    macro_rules! test_compile {
        ($zkvm:ident, $program:literal) => {
            use crate::{
                EreDockerizedCompiler, EreDockerizedzkVM, ErezkVM, SerializedProgram, workspace_dir,
            };
            use std::sync::{Mutex, MutexGuard, OnceLock};
            use test_utils::host::*;
            use zkvm_interface::{Compiler, ProverResourceType};

            fn program() -> &'static SerializedProgram {
                static PROGRAM: OnceLock<SerializedProgram> = OnceLock::new();
                PROGRAM.get_or_init(|| {
                    let zkvm = ErezkVM::$zkvm;
                    let guest_directory = testing_guest_directory(zkvm.as_str(), $program);
                    EreDockerizedCompiler::new(zkvm, workspace_dir())
                        .unwrap()
                        .compile(&guest_directory)
                        .unwrap()
                })
            }

            #[allow(dead_code)]
            fn zkvm() -> (MutexGuard<'static, ()>, EreDockerizedzkVM) {
                static LOCK: Mutex<()> = Mutex::new(());
                let guard = LOCK.lock().unwrap();
                let zkvm = ErezkVM::$zkvm;
                let zkvm = EreDockerizedzkVM::new(zkvm, program().clone(), ProverResourceType::Cpu)
                    .unwrap();
                (guard, zkvm)
            }

            #[test]
            fn test_compile() {
                let program = program();

                assert!(!program.0.is_empty(), "Program should not be empty");
            }
        };
    }

    macro_rules! test_execute {
        ($zkvm:ident, $io:expr) => {
            #[test]
            fn test_execute() {
                let (_guard, zkvm) = zkvm();
                run_zkvm_execute(&zkvm, &$io);
                drop(zkvm);
            }
        };
    }

    macro_rules! test_prove {
        ($zkvm:ident, $io:expr) => {
            #[test]
            fn test_prove() {
                let (_guard, zkvm) = zkvm();
                run_zkvm_prove(&zkvm, &$io);
                drop(zkvm);
            }
        };
    }

    mod jolt {
        test_compile!(Jolt, "basic");
    }

    mod miden {
        test_compile!(Miden, "fib");
    }

    mod nexus {
        test_compile!(Nexus, "basic");
    }

    mod openvm {
        test_compile!(OpenVM, "basic");
        test_execute!(OpenVM, BasicProgramIo::valid().into_output_hashed_io());
        test_prove!(OpenVM, BasicProgramIo::valid().into_output_hashed_io());
    }

    mod pico {
        test_compile!(Pico, "basic");
        test_execute!(Pico, BasicProgramIo::valid());
        test_prove!(Pico, BasicProgramIo::valid());
    }

    mod risc0 {
        test_compile!(Risc0, "basic");
        test_execute!(Risc0, BasicProgramIo::valid());
        test_prove!(Risc0, BasicProgramIo::valid());
    }

    mod sp1 {
        test_compile!(SP1, "basic");
        test_execute!(SP1, BasicProgramIo::valid());
        test_prove!(SP1, BasicProgramIo::valid());
    }

    mod ziren {
        test_compile!(Ziren, "basic");
        test_execute!(Ziren, BasicProgramIo::valid());
        test_prove!(Ziren, BasicProgramIo::valid());
    }

    mod zisk {
        test_compile!(Zisk, "basic");
        test_execute!(Zisk, BasicProgramIo::valid().into_output_hashed_io());
        test_prove!(Zisk, BasicProgramIo::valid().into_output_hashed_io());
    }
}

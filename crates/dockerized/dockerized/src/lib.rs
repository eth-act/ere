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
//! use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};
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
//! // Serialize input
//! let input = 42u32.to_le_bytes();
//!
//! // Execute program
//! let (public_values, execution_report) = zkvm.execute(&input)?;
//! println!("Execution cycles: {}", execution_report.total_num_cycles);
//!
//! // Generate proof
//! let (public_values, proof, proving_report) = zkvm.prove(&input, ProofKind::Compressed)?;
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
    error::DockerizedError,
};
use ere_server::client::{Url, zkVMClient};
use ere_zkvm_interface::{
    Compiler, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, ProverResourceType,
    PublicValues, zkVM, zkVMError,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fmt::{self, Display, Formatter},
    fs, iter,
    path::{Path, PathBuf},
    str::FromStr,
};
use tempfile::TempDir;
use tracing::error;

include!(concat!(env!("OUT_DIR"), "/crate_version.rs"));
include!(concat!(env!("OUT_DIR"), "/zkvm_sdk_version_impl.rs"));

pub mod cuda;
pub mod docker;
pub mod error;

/// Offset of port used for `ere-server` for [`ErezkVM`]s.
const ERE_SERVER_PORT_OFFSET: u16 = 4174;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErezkVM {
    Airbender,
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
            Self::Airbender => "airbender",
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
            (true, Self::Airbender | Self::OpenVM | Self::Risc0 | Self::Zisk) => "-cuda",
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
    pub fn build_docker_image(&self, gpu: bool) -> Result<(), DockerizedError> {
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
                .map_err(DockerizedError::DockerBuildCmd)?;
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
                    Self::Airbender | Self::OpenVM | Self::Risc0 | Self::Zisk => {
                        if let Some(cuda_arch) = cuda_arch {
                            cmd = cmd.build_arg("CUDA_ARCH", cuda_arch)
                        }
                    }
                    _ => {}
                }
            }

            cmd.exec(&workspace_dir)
                .map_err(DockerizedError::DockerBuildCmd)?;
        }

        // Build `ere-compiler-{zkvm}`
        if force_rebuild || !docker_image_exists(self.compiler_zkvm_image(CRATE_VERSION))? {
            DockerBuildCmd::new()
                .file(docker_dir.join(self.as_str()).join("Dockerfile.compiler"))
                .tag(self.compiler_zkvm_image(CRATE_VERSION))
                .tag(self.compiler_zkvm_image("latest"))
                .build_arg("BASE_ZKVM_IMAGE", self.base_zkvm_image(CRATE_VERSION, gpu))
                .exec(&workspace_dir)
                .map_err(DockerizedError::DockerBuildCmd)?;
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
                .map_err(DockerizedError::DockerBuildCmd)?;
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
    ) -> Result<ServerContainer, DockerizedError> {
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
            Self::Risc0 => cmd
                .inherit_env("RISC0_SEGMENT_PO2")
                .inherit_env("RISC0_KECCAK_PO2"),
            // ZisK uses shared memory to exchange data between processes, it
            // requires at least 8G shared memory, here we set 16G for safety.
            Self::Zisk => cmd
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
                Self::Airbender => cmd.gpus("all"),
                Self::OpenVM => cmd.gpus("all"),
                // SP1 runs docker command to spin up the server to do GPU
                // proving, to give the client access to the prover service, we
                // need to use the host networking driver.
                Self::SP1 => cmd.mount_docker_socket().network("host"),
                Self::Risc0 => cmd.gpus("all").inherit_env("RISC0_DEFAULT_PROVER_NUM_GPUS"),
                Self::Zisk => cmd.gpus("all"),
                _ => cmd,
            }
        }

        let tempdir = TempDir::new()
            .map_err(|err| DockerizedError::io(err, "Failed to create temporary directory"))?;

        // zkVM specific options needed for proving Groth16 proof.
        cmd = match self {
            // Risc0 and SP1 runs docker command to prove Groth16 proof, and
            // they pass the input by mounting temporary directory. Here we
            // create a temporary directory and mount it on the top level, so
            // the volume could be shared, and override `TMPDIR` so we don't
            // need to mount the whole `/tmp`.
            Self::Risc0 => cmd
                .mount_docker_socket()
                .env("TMPDIR", tempdir.path().to_string_lossy())
                .volume(tempdir.path(), tempdir.path()),
            Self::SP1 => {
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
            .map_err(DockerizedError::DockerRunCmd)?;

        Ok(ServerContainer { name, tempdir })
    }
}

impl FromStr for ErezkVM {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "airbender" => Self::Airbender,
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
    pub fn new(zkvm: ErezkVM, mount_directory: impl AsRef<Path>) -> Result<Self, DockerizedError> {
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
    type Error = DockerizedError;
    type Program = SerializedProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let guest_relative_path = guest_directory
            .strip_prefix(&self.mount_directory)
            .map_err(|_| DockerizedError::GuestNotInMountingDirecty {
                mounting_directory: self.mount_directory.to_path_buf(),
                guest_directory: guest_directory.to_path_buf(),
            })?;
        let guest_path_in_docker = PathBuf::from("/guest").join(guest_relative_path);

        let tempdir = TempDir::new()
            .map_err(|err| DockerizedError::io(err, "Failed to create temporary directory"))?;

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
        .map_err(DockerizedError::DockerRunCmd)?;

        let program_path = tempdir.path().join("program");
        let program = fs::read(&program_path).map_err(|err| {
            DockerizedError::io(
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
    fn execute(&self, input: &[u8]) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let (public_values, report) =
            block_on(self.client.execute(input.to_vec())).map_err(DockerizedError::from)?;

        Ok((public_values, report))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        let (public_values, proof, report) =
            block_on(self.client.prove(input.to_vec(), proof_kind))
                .map_err(DockerizedError::from)?;

        Ok((public_values, proof, report))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let public_values = block_on(self.client.verify(proof)).map_err(DockerizedError::from)?;

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        self.zkvm.as_str()
    }

    fn sdk_version(&self) -> &'static str {
        self.zkvm.sdk_version()
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
    dir.pop();
    dir.canonicalize().unwrap()
}

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
}

#[cfg(test)]
mod test {
    use crate::{
        DockerizedError, EreDockerizedCompiler, EreDockerizedzkVM, ErezkVM, SerializedProgram,
        workspace_dir,
    };
    use ere_test_utils::{host::*, program::basic::BasicProgramInput};
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM, zkVMError};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    macro_rules! test_compile {
        ($zkvm:ident, $program:literal) => {
            use super::*;

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
        ($zkvm:ident, $valid_test_case:expr, $invalid_test_cases:expr) => {
            #[test]
            fn test_execute() {
                let (_guard, zkvm) = zkvm();

                // Valid test case
                run_zkvm_execute(&zkvm, &$valid_test_case);

                // Invalid test cases
                for input in $invalid_test_cases {
                    let Err(zkVMError::Other(err)) = zkvm.execute(&input) else {
                        unreachable!();
                    };
                    assert!(
                        matches!(
                            err.downcast_ref::<DockerizedError>().unwrap(),
                            DockerizedError::zkVM(_)
                        ),
                        "Unexpected err: {err:?}"
                    );
                }

                drop(zkvm);
            }
        };
    }

    macro_rules! test_prove {
        ($zkvm:ident, $valid_test_case:expr, $invalid_test_cases:expr) => {
            #[test]
            fn test_prove() {
                let (_guard, zkvm) = zkvm();

                // Valid test case
                run_zkvm_prove(&zkvm, &$valid_test_case);

                // Invalid test cases
                for input in $invalid_test_cases {
                    let Err(zkVMError::Other(err)) = zkvm.prove(&input, ProofKind::default())
                    else {
                        unreachable!();
                    };
                    assert!(
                        matches!(
                            err.downcast_ref::<DockerizedError>().unwrap(),
                            DockerizedError::zkVM(_)
                        ),
                        "Unexpected err: {err:?}"
                    );
                }

                drop(zkvm);
            }
        };
    }

    mod airbender {
        test_compile!(Airbender, "basic");
        test_execute!(
            Airbender,
            BasicProgramInput::valid().into_output_sha256(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            Airbender,
            BasicProgramInput::valid().into_output_sha256(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod jolt {
        test_compile!(Jolt, "basic");
    }

    mod miden {
        test_compile!(Miden, "fib");
    }

    mod nexus {
        test_compile!(Nexus, "basic");
        test_execute!(
            Nexus,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            Nexus,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod openvm {
        test_compile!(OpenVM, "basic");
        test_execute!(
            OpenVM,
            BasicProgramInput::valid().into_output_sha256(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            OpenVM,
            BasicProgramInput::valid().into_output_sha256(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod pico {
        test_compile!(Pico, "basic");
        test_execute!(
            Pico,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            Pico,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod risc0 {
        test_compile!(Risc0, "basic");
        test_execute!(
            Risc0,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            Risc0,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod sp1 {
        test_compile!(SP1, "basic");
        test_execute!(
            SP1,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            SP1,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod ziren {
        test_compile!(Ziren, "basic");
        test_execute!(
            Ziren,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            Ziren,
            BasicProgramInput::valid(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod zisk {
        test_compile!(Zisk, "basic");
        test_execute!(
            Zisk,
            BasicProgramInput::valid().into_output_sha256(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
        test_prove!(
            Zisk,
            BasicProgramInput::valid().into_output_sha256(),
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }
}

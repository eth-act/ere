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
//! It builds 3 Docker images in sequence if they don't exist:
//! 1. `ere-base:{version}` - Base image with common dependencies
//! 2. `ere-base-{zkvm}:{version}` - zkVM-specific base image with the zkVM SDK
//! 3. `ere-cli-{zkvm}:{version}` - CLI image with the `ere-cli` binary built
//!    with the selected zkVM feature
//!
//! To force rebuild all images, set the environment variable `ERE_FORCE_REBUILD_DOCKER_IMAGE=true`.
//!
//! ## Example
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use ere_dockerized::{EreDockerizedCompiler, EreDockerizedzkVM, ErezkVM};
//! use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};
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
//! let (public_values, proof, proving_report) = zkvm.prove(&inputs)?;
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
    docker::{DockerBuildCmd, DockerRunCmd, docker_image_exists},
    error::{CommonError, CompileError, DockerizedError, ExecuteError, ProveError, VerifyError},
};
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
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProverResourceType,
    PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/crate_version.rs"));
include!(concat!(env!("OUT_DIR"), "/zkvm_sdk_version_impl.rs"));

pub mod cuda;
pub mod docker;
pub mod error;
pub mod input;
pub mod output;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErezkVM {
    Jolt,
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
            Self::Nexus => "nexus",
            Self::OpenVM => "openvm",
            Self::Pico => "pico",
            Self::Risc0 => "risc0",
            Self::SP1 => "sp1",
            Self::Ziren => "ziren",
            Self::Zisk => "zisk",
        }
    }

    pub fn base_tag(&self, version: &str) -> String {
        format!("ere-base:{version}")
    }

    pub fn base_zkvm_tag(&self, version: &str) -> String {
        format!("ere-base-{self}:{version}")
    }

    pub fn cli_zkvm_tag(&self, version: &str) -> String {
        format!("ere-cli-{self}:{version}")
    }

    /// This method builds 3 Docker images in sequence:
    /// 1. `ere-base:{version}`: Base image with common dependencies
    /// 2. `ere-base-{zkvm}:{version}`: zkVM-specific base image with the zkVM SDK
    /// 3. `ere-cli-{zkvm}:{version}`: CLI image with the `ere-cli` binary built with feature `{zkvm}`
    ///
    /// Images are cached and only rebuilt if they don't exist or if the
    /// `ERE_FORCE_REBUILD_DOCKER_IMAGE=true` environment variable is set.
    pub fn build_docker_image(&self) -> Result<(), CommonError> {
        let workspace_dir = workspace_dir();

        let force_rebuild = env::var("ERE_FORCE_REBUILD_DOCKER_IMAGE")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or_default();

        if force_rebuild || !docker_image_exists(self.base_tag(CRATE_VERSION))? {
            DockerBuildCmd::new()
                .file(
                    workspace_dir
                        .join("docker")
                        .join("base")
                        .join("Dockerfile.base"),
                )
                .tag(self.base_tag(CRATE_VERSION))
                .tag(self.base_tag("latest"))
                .exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        if force_rebuild || !docker_image_exists(self.base_zkvm_tag(CRATE_VERSION))? {
            let mut cmd = DockerBuildCmd::new()
                .file(
                    workspace_dir
                        .join("docker")
                        .join(self.as_str())
                        .join("Dockerfile"),
                )
                .tag(self.base_zkvm_tag(CRATE_VERSION))
                .tag(self.base_zkvm_tag("latest"))
                .build_arg("BASE_IMAGE_TAG", self.base_tag(CRATE_VERSION));

            let cuda_arch = cuda_arch();
            match self {
                ErezkVM::OpenVM => {
                    // OpenVM takes only the numeric part.
                    if let Some(cuda_arch) = cuda_arch {
                        cmd = cmd.build_arg("CUDA_ARCH", cuda_arch.replace("sm_", ""))
                    }
                }
                ErezkVM::Zisk => {
                    if let Some(cuda_arch) = cuda_arch {
                        cmd = cmd.build_arg("CUDA_ARCH", cuda_arch)
                    }
                }
                _ => {}
            }

            cmd.exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        if force_rebuild || !docker_image_exists(self.cli_zkvm_tag(CRATE_VERSION))? {
            DockerBuildCmd::new()
                .file(workspace_dir.join("docker").join("cli").join("Dockerfile"))
                .tag(self.cli_zkvm_tag(CRATE_VERSION))
                .tag(self.cli_zkvm_tag("latest"))
                .build_arg("BASE_ZKVM_IMAGE_TAG", self.base_zkvm_tag(CRATE_VERSION))
                .build_arg("ZKVM", self.as_str())
                .exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        Ok(())
    }
}

impl FromStr for ErezkVM {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "jolt" => Self::Jolt,
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
        zkvm.build_docker_image()?;
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

        let mut cmd = DockerRunCmd::new(self.zkvm.cli_zkvm_tag(CRATE_VERSION))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .volume(&self.mount_directory, "/guest")
            .volume(tempdir.path(), "/guest-output");

        cmd = match self.zkvm {
            // OpenVM allows to select Rust toolchain for guest compilation.
            ErezkVM::OpenVM => cmd.inherit_env("OPENVM_RUST_TOOLCHAIN"),
            _ => cmd,
        };

        cmd.exec([
            "compile",
            "--guest-path",
            guest_path_in_docker.to_string_lossy().as_ref(),
            "--program-path",
            "/guest-output/program",
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

pub struct EreDockerizedzkVM {
    zkvm: ErezkVM,
    program: SerializedProgram,
    resource: ProverResourceType,
}

impl EreDockerizedzkVM {
    pub fn new(
        zkvm: ErezkVM,
        program: SerializedProgram,
        resource: ProverResourceType,
    ) -> Result<Self, zkVMError> {
        zkvm.build_docker_image()?;
        Ok(Self {
            zkvm,
            program,
            resource,
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
        let tempdir = TempDir::new()
            .map_err(|err| CommonError::io(err, "Failed to create temporary directory"))
            .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        fs::write(tempdir.path().join("program"), &self.program.0)
            .map_err(|err| CommonError::io(err, "Failed to write program"))
            .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        fs::write(
            tempdir.path().join("input"),
            self.zkvm
                .serialize_inputs(inputs)
                .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?,
        )
        .map_err(|err| CommonError::io(err, "Failed to write input"))
        .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        DockerRunCmd::new(self.zkvm.cli_zkvm_tag(CRATE_VERSION))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .volume(tempdir.path(), "/workspace")
            .exec([
                "execute",
                "--program-path",
                "/workspace/program",
                "--input-path",
                "/workspace/input",
                "--public-values-path",
                "/workspace/public_values",
                "--report-path",
                "/workspace/report",
            ])
            .map_err(CommonError::DockerRunCmd)
            .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        let public_values = fs::read(tempdir.path().join("public_values"))
            .map_err(|err| CommonError::io(err, "Failed to read public_values"))
            .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;
        let report_bytes = fs::read(tempdir.path().join("report"))
            .map_err(|err| CommonError::io(err, "Failed to read report"))
            .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        let report = bincode::deserialize(&report_bytes)
            .map_err(|err| CommonError::serilization(err, "Failed to deserialize report"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;
        Ok((public_values, report))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        let tempdir = TempDir::new()
            .map_err(|err| CommonError::io(err, "Failed to create temporary directory"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;

        fs::write(tempdir.path().join("program"), &self.program.0)
            .map_err(|err| CommonError::io(err, "Failed to write program"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;
        fs::write(
            tempdir.path().join("input"),
            self.zkvm
                .serialize_inputs(inputs)
                .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?,
        )
        .map_err(|err| CommonError::io(err, "Failed to write input"))
        .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;

        let mut cmd = DockerRunCmd::new(self.zkvm.cli_zkvm_tag(CRATE_VERSION))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .volume(tempdir.path(), "/workspace");

        // zkVM specific options
        cmd = match self.zkvm {
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
        if matches!(self.resource, ProverResourceType::Gpu) {
            cmd = match self.zkvm {
                ErezkVM::OpenVM => cmd.gpus("all"),
                // SP1's and Risc0's GPU proving requires Docker to start GPU prover
                // service, to give the client access to the prover service, we need
                // to use the host networking driver.
                // The `--gpus` flags will be set when the GPU prover service is
                // spin up, so we don't need to set here.
                ErezkVM::SP1 => cmd.mount_docker_socket().network("host"),
                ErezkVM::Risc0 => cmd.gpus("all").inherit_env("RISC0_DEFAULT_PROVER_NUM_GPUS"),
                ErezkVM::Zisk => cmd.gpus("all"),
                _ => cmd,
            }
        }

        cmd.exec(
            iter::empty()
                .chain([
                    "prove",
                    "--program-path",
                    "/workspace/program",
                    "--input-path",
                    "/workspace/input",
                    "--public-values-path",
                    "/workspace/public_values",
                    "--proof-path",
                    "/workspace/proof",
                    "--report-path",
                    "/workspace/report",
                ])
                .chain(self.resource.to_args()),
        )
        .map_err(CommonError::DockerRunCmd)
        .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;

        let public_values = fs::read(tempdir.path().join("public_values"))
            .map_err(|err| CommonError::io(err, "Failed to read public_values"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;
        let proof = fs::read(tempdir.path().join("proof"))
            .map_err(|err| CommonError::io(err, "Failed to read proof"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;
        let report_bytes = fs::read(tempdir.path().join("report"))
            .map_err(|err| CommonError::io(err, "Failed to read report"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;
        let report = bincode::deserialize(&report_bytes)
            .map_err(|err| CommonError::serilization(err, "Failed to deserialize report"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;
        Ok((public_values, proof, report))
    }

    fn verify(&self, proof: &[u8]) -> Result<PublicValues, zkVMError> {
        let tempdir = TempDir::new()
            .map_err(|err| CommonError::io(err, "Failed to create temporary directory"))
            .map_err(|err| DockerizedError::Verify(VerifyError::Common(err)))?;

        fs::write(tempdir.path().join("program"), &self.program.0)
            .map_err(|err| CommonError::io(err, "Failed to write program"))
            .map_err(|err| DockerizedError::Verify(VerifyError::Common(err)))?;
        fs::write(tempdir.path().join("proof"), proof)
            .map_err(|err| CommonError::io(err, "Failed to write proof"))
            .map_err(|err| DockerizedError::Verify(VerifyError::Common(err)))?;

        DockerRunCmd::new(self.zkvm.cli_zkvm_tag(CRATE_VERSION))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .volume(tempdir.path(), "/workspace")
            .exec([
                "verify",
                "--program-path",
                "/workspace/program",
                "--proof-path",
                "/workspace/proof",
                "--public-values-path",
                "/workspace/public_values",
            ])
            .map_err(CommonError::DockerRunCmd)
            .map_err(|err| DockerizedError::Verify(VerifyError::Common(err)))?;

        let public_values = fs::read(tempdir.path().join("public_values"))
            .map_err(|err| CommonError::io(err, "Failed to read public_values"))
            .map_err(|err| DockerizedError::Verify(VerifyError::Common(err)))?;

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

fn workspace_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.canonicalize().unwrap()
}

#[cfg(test)]
mod test {
    use crate::{EreDockerizedCompiler, EreDockerizedzkVM, ErezkVM, workspace_dir};
    use test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };
    use zkvm_interface::{Compiler, ProverResourceType};

    // TODO: Test other ere-{zkvm} when they are end-to-end ready:
    //       - ere-jolt
    //       - ere-nexus

    #[test]
    fn dockerized_openvm() {
        let zkvm = ErezkVM::OpenVM;

        let guest_directory = testing_guest_directory(zkvm.as_str(), "basic");
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .unwrap()
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid().into_output_hashed_io();
        run_zkvm_execute(&zkvm, &io);
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn dockerized_pico() {
        let zkvm = ErezkVM::Pico;

        let guest_directory = testing_guest_directory(zkvm.as_str(), "basic");
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .unwrap()
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn dockerized_risc0() {
        let zkvm = ErezkVM::Risc0;

        let guest_directory = testing_guest_directory(zkvm.as_str(), "basic");
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .unwrap()
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn dockerized_sp1() {
        let zkvm = ErezkVM::SP1;

        let guest_directory = testing_guest_directory(zkvm.as_str(), "basic");
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .unwrap()
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn dockerized_ziren() {
        let zkvm = ErezkVM::Ziren;

        let guest_directory = testing_guest_directory(zkvm.as_str(), "basic");
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .unwrap()
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn dockerized_zisk() {
        let zkvm = ErezkVM::Zisk;

        let guest_directory = testing_guest_directory(zkvm.as_str(), "basic");
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .unwrap()
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid().into_output_hashed_io();
        run_zkvm_execute(&zkvm, &io);
        run_zkvm_prove(&zkvm, &io);
    }
}

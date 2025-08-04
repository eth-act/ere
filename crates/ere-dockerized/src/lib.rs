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
//!     with the selected zkVM feature
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
//! // Compile a guest program
//! let compiler = EreDockerizedCompiler::new(ErezkVM::SP1, "mounting/directory");
//! let guest_path = Path::new("relative/path/to/guest/program");
//! let program = compiler.compile(&guest_path)?;
//!
//! // Create zkVM instance
//! let zkvm = EreDockerizedzkVM::new(
//!     ErezkVM::SP1,
//!     program,
//!     ProverResourceType::Cpu
//! )?;
//!
//! // Prepare inputs
//! let mut inputs = Input::new();
//! inputs.write(42u32);
//! inputs.write(100u16);
//!
//! // Execute program
//! let execution_report = zkvm.execute(&inputs)?;
//! println!("Execution cycles: {}", execution_report.total_num_cycles);
//!
//! // Generate proof
//! let (proof, proving_report) = zkvm.prove(&inputs)?;
//! println!("Proof generated in: {:?}", proving_report.proving_time);
//!
//! // Verify proof
//! zkvm.verify(&proof)?;
//! println!("Proof verified successfully!");
//! # Ok(())
//! # }
//! ```

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    docker::{DockerBuildCmd, DockerRunCmd, docker_image_exists},
    error::{CommonError, CompileError, DockerizedError, ExecuteError, ProveError, VerifyError},
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
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, ProverResourceType, zkVM,
    zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/crate_version.rs"));
include!(concat!(env!("OUT_DIR"), "/zkvm_sdk_version_impl.rs"));

pub mod docker;
pub mod error;
pub mod input;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErezkVM {
    Jolt,
    Nexus,
    OpenVM,
    Pico,
    Risc0,
    SP1,
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
    /// 1. `ere-base:latest`: Base image with common dependencies
    /// 2. `ere-base-{zkvm}:latest`: zkVM-specific base image with the zkVM SDK
    /// 3. `ere-cli-{zkvm}:latest`: CLI image with the `ere-cli` binary built with feature `{zkvm}`
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
            DockerBuildCmd::new()
                .file(
                    workspace_dir
                        .join("docker")
                        .join(self.as_str())
                        .join("Dockerfile"),
                )
                .tag(self.base_zkvm_tag(CRATE_VERSION))
                .tag(self.base_zkvm_tag("latest"))
                .bulid_arg("BASE_IMAGE_TAG", self.base_tag(CRATE_VERSION))
                .exec(&workspace_dir)
                .map_err(CommonError::DockerBuildCmd)?;
        }

        if force_rebuild || !docker_image_exists(self.cli_zkvm_tag(CRATE_VERSION))? {
            DockerBuildCmd::new()
                .file(workspace_dir.join("docker").join("cli").join("Dockerfile"))
                .tag(self.cli_zkvm_tag(CRATE_VERSION))
                .tag(self.cli_zkvm_tag("latest"))
                .bulid_arg("BASE_ZKVM_IMAGE_TAG", self.base_zkvm_tag(CRATE_VERSION))
                .bulid_arg("ZKVM", self.as_str())
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
    pub fn new(zkvm: ErezkVM, mount_directory: impl AsRef<Path>) -> Self {
        Self {
            zkvm,
            mount_directory: mount_directory.as_ref().to_path_buf(),
        }
    }
}

/// Wrapper for serialized program.
#[derive(Clone, Serialize, Deserialize)]
pub struct SerializedProgram(Vec<u8>);

impl Compiler for EreDockerizedCompiler {
    type Error = CompileError;
    type Program = SerializedProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        self.zkvm.build_docker_image()?;

        let guest_relative_path = guest_directory
            .strip_prefix(&self.mount_directory)
            .map_err(|_| CompileError::GuestNotInMountingDirecty {
                mounting_directory: self.mount_directory.to_path_buf(),
                guest_directory: guest_directory.to_path_buf(),
            })?;
        let guest_path_in_docker = PathBuf::from("/guest").join(guest_relative_path);

        let tempdir = TempDir::new()
            .map_err(|err| CommonError::io(err, "Failed to create temporary directory"))?;

        DockerRunCmd::new(self.zkvm.cli_zkvm_tag(CRATE_VERSION))
            .rm()
            .volume(&self.mount_directory, "/guest")
            .volume(tempdir.path(), "/guest-output")
            .exec([
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
}

impl zkVM for EreDockerizedzkVM {
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, zkVMError> {
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

        let mut cmd = DockerRunCmd::new(self.zkvm.cli_zkvm_tag(CRATE_VERSION))
            .rm()
            .volume(tempdir.path(), "/workspace");

        if matches!(self.resource, ProverResourceType::Gpu) {
            cmd = cmd.gpus("all")
        }

        cmd.exec(
            iter::empty()
                .chain([
                    "execute",
                    "--program-path",
                    "/workspace/program",
                    "--input-path",
                    "/workspace/input",
                    "--report-path",
                    "/workspace/report",
                ])
                .chain(self.resource.to_args()),
        )
        .map_err(CommonError::DockerRunCmd)
        .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        let report_bytes = fs::read(tempdir.path().join("report"))
            .map_err(|err| CommonError::io(err, "Failed to read report"))
            .map_err(|err| DockerizedError::Execute(ExecuteError::Common(err)))?;

        let report = bincode::deserialize(&report_bytes)
            .map_err(|err| CommonError::serilization(err, "Failed to deserialize report"))
            .map_err(|err| DockerizedError::Prove(ProveError::Common(err)))?;
        Ok(report)
    }

    fn prove(&self, inputs: &Input) -> Result<(Vec<u8>, ProgramProvingReport), zkVMError> {
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
            .volume(tempdir.path(), "/workspace");

        if matches!(self.resource, ProverResourceType::Gpu) {
            cmd = cmd.gpus("all")
        }

        cmd.exec(
            iter::empty()
                .chain([
                    "prove",
                    "--program-path",
                    "/workspace/program",
                    "--input-path",
                    "/workspace/input",
                    "--proof-path",
                    "/workspace/proof",
                    "--report-path",
                    "/workspace/report",
                ])
                .chain(self.resource.to_args()),
        )
        .map_err(CommonError::DockerRunCmd)
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
        Ok((proof, report))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError> {
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
            .volume(tempdir.path(), "/workspace")
            .exec([
                "verify",
                "--program-path",
                "/workspace/program",
                "--proof-path",
                "/workspace/proof",
            ])
            .map_err(CommonError::DockerRunCmd)
            .map_err(|err| DockerizedError::Verify(VerifyError::Common(err)))?;

        Ok(())
    }

    fn name(&self) -> &'static str {
        self.zkvm.as_str()
    }

    fn sdk_version(&self) -> &'static str {
        self.zkvm.sdk_version()
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
    use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

    // TODO: Test other ere-{zkvm} when they are end-to-end ready:
    //       - ere-jolt
    //       - ere-nexus
    //       - ere-pico

    #[test]
    fn dockerized_openvm() {
        let zkvm = ErezkVM::OpenVM;

        let guest_directory = workspace_dir().join(format!("tests/{zkvm}/compile/basic"));
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let mut inputs = Input::new();
        inputs.write(42u64);

        let _report = zkvm.execute(&inputs).unwrap();

        let (proof, _report) = zkvm.prove(&inputs).unwrap();

        zkvm.verify(&proof).unwrap();
    }

    #[test]
    fn dockerized_risc0() {
        let zkvm = ErezkVM::Risc0;

        let guest_directory = workspace_dir().join(format!("tests/{zkvm}/compile/basic"));
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let mut inputs = Input::new();
        inputs.write(42u32);

        let _report = zkvm.execute(&inputs).unwrap();

        let (proof, _report) = zkvm.prove(&inputs).unwrap();

        zkvm.verify(&proof).unwrap();
    }

    #[test]
    fn dockerized_sp1() {
        let zkvm = ErezkVM::SP1;

        let guest_directory = workspace_dir().join(format!("tests/{zkvm}/prove/basic"));
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let mut inputs = Input::new();
        inputs.write(42u32);
        inputs.write(42u16);

        let _report = zkvm.execute(&inputs).unwrap();

        let (proof, _report) = zkvm.prove(&inputs).unwrap();

        zkvm.verify(&proof).unwrap();
    }

    #[test]
    fn dockerized_zisk() {
        let zkvm = ErezkVM::Zisk;

        let guest_directory = workspace_dir().join(format!("tests/{zkvm}/prove/basic"));
        let program = EreDockerizedCompiler::new(zkvm, workspace_dir())
            .compile(&guest_directory)
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let mut inputs = Input::new();
        inputs.write(42u32);
        inputs.write(42u16);

        let _report = zkvm.execute(&inputs).unwrap();

        let (proof, _report) = zkvm.prove(&inputs).unwrap();

        zkvm.verify(&proof).unwrap();
    }
}

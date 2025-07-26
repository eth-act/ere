use crate::docker::{DockerBuildCmd, DockerFile, DockerRunCmd};
use std::{
    fmt::{self, Display, Formatter},
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};
use tempfile::TempDir;
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, ProverResourceType, zkVM,
    zkVMError,
};

pub mod docker;
pub mod input;

#[derive(Clone, Copy, Debug)]
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

    pub fn base_tag(&self) -> String {
        format!("ere-base-{self}:latest")
    }

    pub fn cli_tag(&self) -> String {
        format!("ere-cli-{self}:latest")
    }

    pub fn build_docker_image(&self) -> Result<(), io::Error> {
        let workspace_dir = workspace_dir();
        let base_tag = self.base_tag();
        let cli_tag = self.cli_tag();

        DockerBuildCmd::new()
            .file(
                workspace_dir
                    .join("docker")
                    .join("base")
                    .join("Dockerfile.base"),
            )
            .tag("ere-base:latest")
            .run(&workspace_dir)?;

        DockerBuildCmd::new()
            .file(
                workspace_dir
                    .join("docker")
                    .join(self.as_str())
                    .join("Dockerfile"),
            )
            .tag(&base_tag)
            .run(&workspace_dir)?;

        let (dockerfile_path, _tempdir) = DockerFile::new()
            .from(base_tag)
            .copy(".", "/ere")
            .work_dir("/ere")
            .run([
                "cargo",
                "build",
                "--release",
                "--package",
                "ere-cli",
                "--bin",
                "ere-cli",
                "--features",
                self.as_str(),
            ])
            .run(["cp", "/ere/target/release/ere-cli", "/ere/ere-cli"])
            .run(["cargo", "clean"])
            .entrypoint(["/ere/ere-cli"])
            .into_tempfile()?;

        DockerBuildCmd::new()
            .file(dockerfile_path)
            .tag(cli_tag)
            .run(&workspace_dir)?;

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

pub struct EreDockerizedCompiler(pub ErezkVM);

impl Compiler for EreDockerizedCompiler {
    type Error = io::Error;
    type Program = Vec<u8>;

    fn compile(
        &self,
        mount_directory: &Path,
        guest_relative: &Path,
    ) -> Result<Self::Program, Self::Error> {
        self.0.build_docker_image()?;

        let cli_tag = self.0.cli_tag();

        let tempdir = TempDir::new()?;

        let mut cmd = DockerRunCmd::new(cli_tag)
            .rm()
            .volume(mount_directory, "/guest")
            .volume(tempdir.path(), "/guest-output");

        cmd = match self.0 {
            ErezkVM::Risc0 => cmd.volume("/var/run/docker.sock", "/var/run/docker.sock"),
            _ => cmd,
        };

        cmd.run([
            "compile",
            PathBuf::from("/guest")
                .join(guest_relative)
                .to_string_lossy()
                .as_ref(),
            "/guest-output/program",
        ])?;

        fs::read(tempdir.path().join("program"))
    }
}

pub struct EreDockerizedzkVM {
    zkvm: ErezkVM,
    program: Vec<u8>,
    resource: ProverResourceType,
}

impl EreDockerizedzkVM {
    pub fn new(
        zkvm: ErezkVM,
        program: Vec<u8>,
        resource: ProverResourceType,
    ) -> Result<Self, zkVMError> {
        match resource {
            ProverResourceType::Cpu | ProverResourceType::Gpu => {}
            ProverResourceType::Network(_) => {
                return Err(zkVMError::Other(
                    "Network prover resource type is not supported yet".into(),
                ));
            }
        };
        zkvm.build_docker_image()
            .map_err(|err| zkVMError::Other(err.into()))?;
        Ok(Self {
            zkvm,
            program,
            resource,
        })
    }
}

impl zkVM for EreDockerizedzkVM {
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, zkVMError> {
        let cli_tag = self.zkvm.cli_tag();

        let tempdir = TempDir::new().map_err(|err| zkVMError::Other(err.into()))?;

        fs::write(tempdir.path().join("program"), &self.program)
            .map_err(|err| zkVMError::Other(err.into()))?;
        fs::write(
            tempdir.path().join("input"),
            self.zkvm
                .serialize_inputs(inputs)
                .map_err(|err| zkVMError::Other(err.into()))?,
        )
        .map_err(|err| zkVMError::Other(err.into()))?;

        DockerRunCmd::new(cli_tag)
            .rm()
            .volume(tempdir.path(), "/workspace")
            .run([
                "execute",
                "/workspace/program",
                "/workspace/input",
                "/workspace/report",
            ])
            .map_err(|err| zkVMError::Other(err.into()))?;

        bincode::deserialize(
            &fs::read(tempdir.path().join("report")).map_err(|err| zkVMError::Other(err.into()))?,
        )
        .map_err(|err| zkVMError::Other(err.into()))
    }

    fn prove(&self, inputs: &Input) -> Result<(Vec<u8>, ProgramProvingReport), zkVMError> {
        let cli_tag = self.zkvm.cli_tag();
        let resource = match self.resource {
            ProverResourceType::Cpu => "cpu",
            ProverResourceType::Gpu => "gpu",
            _ => unreachable!(),
        };

        let tempdir = TempDir::new().map_err(|err| zkVMError::Other(err.into()))?;

        fs::write(tempdir.path().join("program"), &self.program)
            .map_err(|err| zkVMError::Other(err.into()))?;
        fs::write(
            tempdir.path().join("input"),
            self.zkvm
                .serialize_inputs(inputs)
                .map_err(|err| zkVMError::Other(err.into()))?,
        )
        .map_err(|err| zkVMError::Other(err.into()))?;

        DockerRunCmd::new(cli_tag)
            .rm()
            .volume(tempdir.path(), "/workspace")
            .run([
                "prove",
                "/workspace/program",
                resource,
                "/workspace/input",
                "/workspace/proof",
                "/workspace/report",
            ])
            .map_err(|err| zkVMError::Other(err.into()))?;

        let proof =
            fs::read(tempdir.path().join("proof")).map_err(|err| zkVMError::Other(err.into()))?;
        let report = bincode::deserialize(
            &fs::read(tempdir.path().join("report")).map_err(|err| zkVMError::Other(err.into()))?,
        )
        .map_err(|err| zkVMError::Other(err.into()))?;
        Ok((proof, report))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError> {
        let cli_tag = self.zkvm.cli_tag();

        let tempdir = TempDir::new().map_err(|err| zkVMError::Other(err.into()))?;

        fs::write(tempdir.path().join("program"), &self.program)
            .map_err(|err| zkVMError::Other(err.into()))?;
        fs::write(tempdir.path().join("proof"), proof)
            .map_err(|err| zkVMError::Other(err.into()))?;

        DockerRunCmd::new(cli_tag)
            .rm()
            .volume(tempdir.path(), "/workspace")
            .run(["verify", "/workspace/program", "/workspace/proof"])
            .map_err(|err| zkVMError::Other(err.into()))
    }

    fn name(&self) -> &'static str {
        self.zkvm.as_str()
    }

    fn sdk_version(&self) -> &'static str {
        // TODO: Find the dependency version, perhaps needs to change the `&'static str` to `String`.
        ""
    }
}

fn workspace_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_PATH"));
    dir.pop();
    dir.pop();
    dir.pop();
    dir.canonicalize().unwrap()
}

#[cfg(test)]
mod test {
    use crate::{EreDockerizedCompiler, EreDockerizedzkVM, ErezkVM, workspace_dir};
    use std::path::PathBuf;
    use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

    #[test]
    fn dockerized_sp1() {
        let zkvm = ErezkVM::SP1;

        let guest_rel_path = PathBuf::from(format!("tests/{zkvm}/prove/basic"));
        let program = EreDockerizedCompiler(zkvm)
            .compile(&workspace_dir(), guest_rel_path.as_ref())
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
    fn dockerized_risc0() {
        let zkvm = ErezkVM::Risc0;

        let guest_rel_path = PathBuf::from(format!("tests/{zkvm}/compile/basic"));
        let program = EreDockerizedCompiler(zkvm)
            .compile(&workspace_dir(), guest_rel_path.as_ref())
            .unwrap();

        let zkvm = EreDockerizedzkVM::new(zkvm, program, ProverResourceType::Cpu).unwrap();

        let mut inputs = Input::new();
        inputs.write(42u32);

        let _report = zkvm.execute(&inputs).unwrap();

        let (proof, _report) = zkvm.prove(&inputs).unwrap();

        zkvm.verify(&proof).unwrap();
    }

    #[test]
    fn dockerized_zisk() {
        let zkvm = ErezkVM::Zisk;

        let guest_rel_path = PathBuf::from(format!("tests/{zkvm}/prove/basic"));
        let program = EreDockerizedCompiler(zkvm)
            .compile(&workspace_dir(), guest_rel_path.as_ref())
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

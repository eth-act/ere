use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid Dockerfile path: {0}")]
    InvalidDockerfilePath(PathBuf),
    #[error("Docker image build failed: {0}")]
    DockerBuildFailed(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Docker image build failed")]
    ImageBuildFailed,
    #[error("Docker is not available. Please ensure Docker is installed and running.")]
    DockerIsNotAvailable,
}

pub fn build_image(compiler_dockerfile: &Path, tag: &str) -> Result<(), Error> {
    // Check that Docker is installed and available
    if Command::new("docker")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        return Err(Error::DockerIsNotAvailable);
    }

    info!(
        "Building Docker image in {} with tag {}",
        compiler_dockerfile.display(),
        tag
    );

    let cargo_workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();

    // Build base image
    info!("Building base Docker image...");
    let dockerfile_base_path = cargo_workspace_dir.join("docker/base/Dockerfile.base");
    let status = Command::new("docker")
        .args([
            "build",
            "-t",
            "ere-base:latest",
            "-f",
            dockerfile_base_path
                .to_str()
                .ok_or_else(|| Error::InvalidDockerfilePath(dockerfile_base_path.clone()))?,
            cargo_workspace_dir.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| Error::DockerBuildFailed(e.into()))?;
    if !status.success() {
        return Err(Error::ImageBuildFailed);
    }

    info!("Building guest compiler image...");
    let dockerfile_path = cargo_workspace_dir.join(compiler_dockerfile);
    let status = Command::new("docker")
        .args([
            "build",
            "-t",
            tag,
            "-f",
            dockerfile_path
                .to_str()
                .ok_or_else(|| Error::InvalidDockerfilePath(dockerfile_path.clone()))?,
            cargo_workspace_dir.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| Error::DockerBuildFailed(e.into()))?;

    if !status.success() {
        return Err(Error::ImageBuildFailed);
    }

    Ok(())
}

#[derive(Debug)]
pub struct DockerRunCommand {
    image: String,
    volumes: Vec<(String, String)>, // (host_path, container_path)
    command: Vec<String>,
    // remove image after running
    remove_after: bool,
}

impl DockerRunCommand {
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            volumes: Vec::new(),
            command: Vec::new(),
            remove_after: false,
        }
    }

    pub fn with_volume(
        mut self,
        host_path: impl Into<String>,
        container_path: impl Into<String>,
    ) -> Self {
        self.volumes.push((host_path.into(), container_path.into()));
        self
    }

    pub fn with_command(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.command.extend(args.into_iter().map(|s| s.into()));
        self
    }

    pub fn remove_after_run(mut self) -> Self {
        self.remove_after = true;
        self
    }

    pub fn to_args(&self) -> Vec<String> {
        let mut args = vec!["run".to_string()];

        if self.remove_after {
            args.push("--rm".to_string());
        }

        for (host_path, container_path) in &self.volumes {
            args.extend(["-v".to_string(), format!("{host_path}:{container_path}")]);
        }

        args.push(self.image.clone());
        args.extend(self.command.iter().cloned());

        args
    }

    pub fn run(&self) -> Result<std::process::ExitStatus, std::io::Error> {
        Command::new("docker").args(self.to_args()).status()
    }
}

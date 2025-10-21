use crate::error::DockerizedError;
use std::{
    env,
    fmt::{self, Display, Formatter},
    io::{self, Write},
    path::Path,
    process::{Child, Command, Stdio},
};

pub const DOCKER_SOCKET: &str = "/var/run/docker.sock";

#[derive(Clone)]
pub struct CmdOption(String, Option<String>);

impl CmdOption {
    pub fn new(key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        Self(to_string(key), Some(to_string(value)))
    }

    pub fn flag(key: impl AsRef<str>) -> Self {
        Self(to_string(key), None)
    }

    pub fn to_args(&self) -> Vec<String> {
        let Self(key, value) = self;
        match value {
            Some(value) => vec![format!("--{key}"), format!("{value}")],
            None => vec![format!("--{key}")],
        }
    }
}

impl Display for CmdOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self(key, value) = self;
        match value {
            Some(value) => write!(f, "--{key} {value}"),
            None => write!(f, "--{key}"),
        }
    }
}

#[derive(Default)]
pub struct DockerBuildCmd {
    options: Vec<CmdOption>,
}

impl DockerBuildCmd {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn option(mut self, key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.options.push(CmdOption::new(key, value));
        self
    }

    pub fn file(self, file: impl AsRef<Path>) -> Self {
        self.option("file", file.as_ref().to_string_lossy())
    }

    pub fn tag(self, tag: impl AsRef<str>) -> Self {
        self.option("tag", tag)
    }

    pub fn build_arg(self, key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.option(
            "build-arg",
            format!("{}={}", to_string(key), to_string(value)),
        )
    }

    pub fn exec(self, context: impl AsRef<Path>) -> Result<(), io::Error> {
        let mut cmd = Command::new("docker");
        cmd.arg("build");
        for option in self.options {
            cmd.args(option.to_args());
        }
        cmd.arg(context.as_ref().to_string_lossy().to_string());

        let status = cmd.status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "Command {cmd:?} failed with status: {status}",
            )));
        }

        Ok(())
    }
}

pub struct DockerRunCmd {
    options: Vec<CmdOption>,
    image: String,
}

impl DockerRunCmd {
    pub fn new(image: String) -> Self {
        Self {
            options: Vec::new(),
            image,
        }
    }

    pub fn flag(mut self, key: impl AsRef<str>) -> Self {
        self.options.push(CmdOption::flag(key));
        self
    }

    pub fn option(mut self, key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.options.push(CmdOption::new(key, value));
        self
    }

    pub fn publish(self, host: impl AsRef<str>, container: impl AsRef<str>) -> Self {
        self.option(
            "publish",
            format!("{}:{}", host.as_ref(), container.as_ref()),
        )
    }

    pub fn volume(self, host: impl AsRef<Path>, container: impl AsRef<Path>) -> Self {
        self.option(
            "volume",
            format!(
                "{}:{}",
                host.as_ref().display(),
                container.as_ref().display(),
            ),
        )
    }

    pub fn env(self, key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.option("env", format!("{}={}", key.as_ref(), value.as_ref()))
    }

    /// Mounts `/var/run/docker.sock` to allow Docker-out-of-Docker (DooD).
    pub fn mount_docker_socket(self) -> Self {
        self.volume(DOCKER_SOCKET, DOCKER_SOCKET)
    }

    pub fn gpus(self, devices: impl AsRef<str>) -> Self {
        self.option("gpus", devices)
    }

    pub fn network(self, name: impl AsRef<str>) -> Self {
        self.option("network", name)
    }

    pub fn name(self, name: impl AsRef<str>) -> Self {
        self.option("name", name)
    }

    /// Inherit environment variable `key` if it's set and valid.
    pub fn inherit_env(self, key: impl AsRef<str>) -> Self {
        let key = key.as_ref();
        match env::var(key) {
            Ok(val) => self.env(key, val),
            Err(_) => self,
        }
    }

    pub fn rm(self) -> Self {
        self.flag("rm")
    }

    pub fn spawn(
        mut self,
        commands: impl IntoIterator<Item: AsRef<str>>,
        stdin: &[u8],
    ) -> Result<Child, io::Error> {
        self = self.flag("interactive");

        let mut cmd = Command::new("docker");
        cmd.arg("run");
        for option in self.options {
            cmd.args(option.to_args());
        }
        cmd.arg(self.image);
        for command in commands {
            cmd.arg(command.as_ref());
        }

        let mut child = cmd.stdin(Stdio::piped()).spawn()?;

        // Write all to stdin then drop to close the pipe.
        child.stdin.take().unwrap().write_all(stdin)?;

        Ok(child)
    }

    pub fn exec(self, commands: impl IntoIterator<Item: AsRef<str>>) -> Result<(), io::Error> {
        let mut cmd = Command::new("docker");
        cmd.arg("run");
        for option in self.options {
            cmd.args(option.to_args());
        }
        cmd.arg(self.image);
        for command in commands {
            cmd.arg(command.as_ref());
        }

        let status = cmd.status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "Command {cmd:?} failed with status: {status}",
            )));
        }

        Ok(())
    }
}

pub fn stop_docker_container(container_name: impl AsRef<str>) -> Result<(), DockerizedError> {
    let output = Command::new("docker")
        .args(["container", "stop", container_name.as_ref()])
        .output()
        .map_err(DockerizedError::DockerContainerCmd)?;

    if String::from_utf8_lossy(&output.stdout).starts_with("Error") {
        return Err(DockerizedError::DockerContainerCmd(io::Error::other(
            format!("Failed to stop container {}", container_name.as_ref()),
        )));
    }

    Ok(())
}

pub fn docker_image_exists(image: impl AsRef<str>) -> Result<bool, DockerizedError> {
    let output = Command::new("docker")
        .args(["images", "--quiet", image.as_ref()])
        .output()
        .map_err(DockerizedError::DockerImageCmd)?;
    // If image exists, image id will be printed hence stdout will be non-empty.
    Ok(!output.stdout.is_empty())
}

fn to_string(s: impl AsRef<str>) -> String {
    s.as_ref().to_string()
}

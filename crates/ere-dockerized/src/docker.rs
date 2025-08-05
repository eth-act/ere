use crate::error::CommonError;
use std::{
    env,
    fmt::{self, Display, Formatter},
    io,
    path::Path,
    process::Command,
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

    pub fn file(mut self, file: impl AsRef<Path>) -> Self {
        self.options
            .push(CmdOption::new("file", file.as_ref().to_string_lossy()));
        self
    }

    pub fn tag(mut self, tag: impl AsRef<str>) -> Self {
        self.options.push(CmdOption::new("tag", tag));
        self
    }

    pub fn bulid_arg(mut self, key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.options.push(CmdOption::new(
            "build-arg",
            format!("{}={}", to_string(key), to_string(value)),
        ));
        self
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

    pub fn volume(mut self, host: impl AsRef<Path>, container: impl AsRef<Path>) -> Self {
        self.options.push(CmdOption::new(
            "volume",
            format!(
                "{}:{}",
                host.as_ref().display(),
                container.as_ref().display(),
            ),
        ));
        self
    }

    /// Mounts `/var/run/docker.sock` to allow Docker-out-of-Docker (DooD).
    pub fn mount_docker_socket(self) -> Self {
        self.volume(DOCKER_SOCKET, DOCKER_SOCKET)
    }

    pub fn gpus(mut self, devices: impl AsRef<str>) -> Self {
        self.options.push(CmdOption::new("gpus", devices));
        self
    }

    pub fn network(mut self, name: impl AsRef<str>) -> Self {
        self.options.push(CmdOption::new("network", name));
        self
    }

    /// Inherit environment variable `key` if it's set and valid.
    pub fn inherit_env(mut self, key: impl AsRef<str>) -> Self {
        let key = key.as_ref();
        if let Ok(val) = env::var(key) {
            self.options
                .push(CmdOption::new("env", format!("{key}={val}")));
        }
        self
    }

    pub fn rm(mut self) -> Self {
        self.options.push(CmdOption::flag("rm"));
        self
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

pub fn docker_image_exists(image: impl AsRef<str>) -> Result<bool, CommonError> {
    let output = Command::new("docker")
        .args(["images", "--quiet", image.as_ref()])
        .output()
        .map_err(CommonError::DockerImageCmd)?;
    // If image exists, image id will be printed hence stdout will be non-empty.
    Ok(!output.stdout.is_empty())
}

fn to_string(s: impl AsRef<str>) -> String {
    s.as_ref().to_string()
}

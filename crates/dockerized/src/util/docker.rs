use ere_zkvm_interface::CommonError;
use std::{
    env,
    fmt::{self, Display, Formatter},
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
};
use tracing::debug;

pub const DOCKER_SOCKET: &str = "/var/run/docker.sock";

#[derive(Clone)]
struct CmdOption(String, Option<String>);

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

    pub fn build_arg_from_env(self, key: impl AsRef<str>) -> Self {
        let key = key.as_ref();
        match env::var(key) {
            Ok(val) => self.build_arg(key, val),
            Err(_) => self,
        }
    }

    pub fn exec(self, context: impl AsRef<Path>) -> Result<(), CommonError> {
        let mut cmd = Command::new("docker");
        cmd.arg("build");
        for option in self.options {
            cmd.args(option.to_args());
        }
        cmd.arg(context.as_ref().to_string_lossy().to_string());

        debug!("Docker build with command: {cmd:?}");

        let status = cmd
            .status()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !status.success() {
            Err(CommonError::command_exit_non_zero(&cmd, status, None))?
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

    pub fn gpus(self) -> Self {
        let devices = env::var("ERE_GPU_DEVICES").unwrap_or_else(|_| "all".to_string());
        self.option("gpus", &devices)
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
    ) -> Result<Child, CommonError> {
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

        debug!("Docker run with command: {cmd:?}");

        let mut child = cmd
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|err| CommonError::command(&cmd, err))?;

        // Write all to stdin then drop to close the pipe.
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin)
            .map_err(|err| CommonError::command(&cmd, err))?;

        Ok(child)
    }

    pub fn exec(self, commands: impl IntoIterator<Item: AsRef<str>>) -> Result<(), CommonError> {
        let mut cmd = Command::new("docker");
        cmd.arg("run");
        for option in self.options {
            cmd.args(option.to_args());
        }
        cmd.arg(self.image);
        for command in commands {
            cmd.arg(command.as_ref());
        }

        debug!("Docker run with command: {cmd:?}");

        let status = cmd
            .status()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !status.success() {
            Err(CommonError::command_exit_non_zero(&cmd, status, None))?
        }

        Ok(())
    }
}

pub fn stop_docker_container(container_name: impl AsRef<str>) -> Result<(), CommonError> {
    let mut cmd = Command::new("docker");
    let output = cmd
        .args(["container", "stop", container_name.as_ref()])
        .output()
        .map_err(|err| CommonError::command(&cmd, err))?;

    if !output.status.success() {
        Err(CommonError::command_exit_non_zero(
            &cmd,
            output.status,
            Some(&output),
        ))?
    }

    Ok(())
}

pub fn docker_container_exists(container_name: impl AsRef<str>) -> Result<bool, CommonError> {
    let mut cmd = Command::new("docker");
    let output = cmd
        .args([
            "ps",
            "--filter",
            &format!("name={}", container_name.as_ref()),
            "--format",
            "{{.Names}}",
        ])
        .output()
        .map_err(|err| CommonError::command(&cmd, err))?;

    if !output.status.success() {
        Err(CommonError::command_exit_non_zero(
            &cmd,
            output.status,
            Some(&output),
        ))?
    }

    // If container exists and is running, its name will be printed
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim() == container_name.as_ref())
}

pub fn docker_image_exists(image: impl AsRef<str>) -> Result<bool, CommonError> {
    let mut cmd = Command::new("docker");
    let output = cmd
        .args(["images", "--quiet", image.as_ref()])
        .output()
        .map_err(|err| CommonError::command(&cmd, err))?;

    if !output.status.success() {
        Err(CommonError::command_exit_non_zero(
            &cmd,
            output.status,
            Some(&output),
        ))?
    }

    // If image exists, image id will be printed hence stdout will be non-empty.
    Ok(!output.stdout.is_empty())
}

pub fn force_rebuild() -> bool {
    env::var_os("ERE_FORCE_REBUILD_DOCKER_IMAGE").is_some()
}

fn to_string(s: impl AsRef<str>) -> String {
    s.as_ref().to_string()
}

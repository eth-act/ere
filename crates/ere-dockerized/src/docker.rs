use std::{
    fmt::{self, Display, Formatter},
    fs::{self},
    io,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

#[derive(Clone)]
pub struct CmdOption(String, Option<String>);

impl CmdOption {
    pub fn new(key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        Self(to_string(key), Some(to_string(value)))
    }

    pub fn flag(key: impl AsRef<str>) -> Self {
        Self(to_string(key), None)
    }
}

impl Display for CmdOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self(key, value) = self;
        match value {
            Some(value) => write!(f, "--{key}={value}"),
            None => write!(f, "--{key}"),
        }
    }
}

#[derive(Clone)]
pub enum DockerInstruction {
    Copy(Vec<CmdOption>, String, String), // COPY {options} {src} {dst}
    Entrypoint(Vec<String>),              // ENTRYPOINT [{commands}]
    Env(String, String),                  // ENV {key}="{value}"
    From(String, Option<String>),         // FROM {image} AS {name}
    Run(Vec<CmdOption>, Vec<String>),     // RUN {options} {commands}
    User(String),                         // USER {uid_gid}
    WorkDir(String),                      // WORKDIR {path}
}

impl Display for DockerInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DockerInstruction::Copy(options, src, dst) => {
                write!(f, "COPY")?;
                for flag in options {
                    write!(f, " {flag}")?;
                }
                write!(f, " {src} {dst}")
            }
            DockerInstruction::Entrypoint(commands) => {
                write!(
                    f,
                    "ENTRYPOINT [{}]",
                    commands
                        .iter()
                        .map(|cmd| format!("\"{cmd}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            DockerInstruction::Env(key, value) => write!(f, "ENV {key}=\"{value}\""),
            DockerInstruction::From(image, name) => match name {
                Some(name) => write!(f, "FROM {image} AS {name}"),
                None => write!(f, "FROM {image}"),
            },
            DockerInstruction::Run(options, commands) => {
                write!(f, "RUN")?;
                for flag in options {
                    write!(f, " {flag}")?;
                }
                write!(f, " {}", commands.join(" "))
            }
            DockerInstruction::User(uid_gid) => write!(f, "USER {uid_gid}"),
            DockerInstruction::WorkDir(path) => write!(f, "WORKDIR {path}"),
        }
    }
}

#[derive(Clone, Default)]
pub struct DockerFile(Vec<DockerInstruction>);

impl DockerFile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entrypoint(mut self, commands: impl IntoIterator<Item: AsRef<str>>) -> Self {
        self.0.push(DockerInstruction::Entrypoint(
            commands.into_iter().map(to_string).collect(),
        ));
        self
    }

    pub fn copy(mut self, src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Self {
        self.0.push(DockerInstruction::Copy(
            Vec::new(),
            to_string(src.as_ref().to_string_lossy()),
            to_string(dst.as_ref().to_string_lossy()),
        ));
        self
    }

    pub fn copy_from(
        mut self,
        name: impl AsRef<str>,
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
    ) -> Self {
        self.0.push(DockerInstruction::Copy(
            vec![CmdOption::new("from", name)],
            to_string(src.as_ref().to_string_lossy()),
            to_string(dst.as_ref().to_string_lossy()),
        ));
        self
    }

    pub fn from(mut self, image: impl AsRef<str>) -> Self {
        self.0.push(DockerInstruction::From(to_string(image), None));
        self
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_as(mut self, image: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        self.0.push(DockerInstruction::From(
            to_string(image),
            to_string(name).into(),
        ));
        self
    }

    pub fn run(mut self, commands: impl IntoIterator<Item: AsRef<str>>) -> Self {
        self.0.push(DockerInstruction::Run(
            Vec::new(),
            commands.into_iter().map(to_string).collect(),
        ));
        self
    }

    pub fn work_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.0.push(DockerInstruction::WorkDir(to_string(
            dir.as_ref().to_string_lossy(),
        )));
        self
    }

    pub fn into_tempfile(self) -> Result<(PathBuf, TempDir), io::Error> {
        let dir = TempDir::new()?;
        let tempfile_path = dir.path().join("Dockerfile");
        fs::write(&tempfile_path, self.to_string())?;
        Ok((tempfile_path, dir))
    }
}

impl Display for DockerFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for inst in &self.0 {
            writeln!(f, "{inst}")?;
        }
        Ok(())
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

    pub fn run(self, context: impl AsRef<Path>) -> Result<(), io::Error> {
        let mut cmd = Command::new("docker");
        cmd.arg("build");
        for flag in self.options {
            cmd.arg(flag.to_string());
        }
        cmd.arg(context.as_ref().to_string_lossy().to_string());

        let status = cmd.status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "Command `{cmd:?}` failed with status: {status}"
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

    pub fn rm(mut self) -> Self {
        self.options.push(CmdOption::flag("rm"));
        self
    }

    pub fn run(self, commands: impl IntoIterator<Item: AsRef<str>>) -> Result<(), io::Error> {
        let mut cmd = Command::new("docker");
        cmd.arg("run");
        for flag in self.options {
            cmd.arg(flag.to_string());
        }
        cmd.arg(self.image);
        for command in commands {
            cmd.arg(command.as_ref());
        }

        let status = cmd.status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "Command {cmd:?} failed with status: {status}"
            )));
        }

        Ok(())
    }
}

fn to_string(s: impl AsRef<str>) -> String {
    s.as_ref().to_string()
}

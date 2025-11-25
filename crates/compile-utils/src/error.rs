use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Output},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommonError {
    #[error("{ctx}: {err}")]
    Io {
        ctx: String,
        #[source]
        err: io::Error,
    },

    #[error("Deserialize {id} with `{lib}` failed: {err}")]
    Deserialize {
        id: String,
        lib: String,
        #[source]
        err: anyhow::Error,
    },

    #[error("Failed to run command `{cmd}`: {err}")]
    Command {
        cmd: String,
        #[source]
        err: io::Error,
    },

    #[error("Command `{cmd}` exit with {status}{stdout}{stderr}",
        stdout = if stdout.is_empty() { String::new() } else { format!("\nstdout: {stdout}") },
        stderr = if stderr.is_empty() { String::new() } else { format!("\nstderr: {stderr}") })]
    CommandExitNonZero {
        cmd: String,
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },

    #[error("`cargo metadata` in {manifest_dir} failed: {err}")]
    CargoMetadata {
        manifest_dir: PathBuf,
        #[source]
        err: cargo_metadata::Error,
    },

    #[error("Root package not found in {manifest_dir}")]
    CargoRootPackageNotFound { manifest_dir: PathBuf },

    #[error("Attempt to get {var} env variable results in {var_error}")]
    Env {
        var: String,
        var_error: std::env::VarError,
    },
}

impl CommonError {
    pub fn io(ctx: impl AsRef<str>, err: io::Error) -> Self {
        let ctx = ctx.as_ref().to_string();
        Self::Io { ctx, err }
    }

    pub fn tempdir(err: io::Error) -> Self {
        Self::io("Failed to create temporary dir", err)
    }

    pub fn create_dir(id: impl AsRef<str>, path: impl AsRef<Path>, err: io::Error) -> Self {
        let (id, path) = (id.as_ref(), path.as_ref().display());
        Self::io(format!("Failed to create dir {id} at {path}"), err)
    }

    pub fn read_file(id: impl AsRef<str>, path: impl AsRef<Path>, err: io::Error) -> Self {
        let (id, path) = (id.as_ref(), path.as_ref().display());
        Self::io(format!("Failed to read {id} from {path}"), err)
    }

    pub fn write_file(id: impl AsRef<str>, path: impl AsRef<Path>, err: io::Error) -> Self {
        let (id, path) = (id.as_ref(), path.as_ref().display());
        Self::io(format!("Failed to write {id} to {path}"), err)
    }

    pub fn deserialize(
        id: impl AsRef<str>,
        lib: impl AsRef<str>,
        err: impl Into<anyhow::Error>,
    ) -> Self {
        let id = id.as_ref().to_string();
        let lib = lib.as_ref().to_string();
        let err = err.into();
        Self::Deserialize { id, lib, err }
    }

    pub fn command(cmd: &Command, err: io::Error) -> Self {
        Self::Command {
            cmd: format!("{cmd:?}"),
            err,
        }
    }

    pub fn command_exit_non_zero(
        cmd: &Command,
        status: ExitStatus,
        output: Option<&Output>,
    ) -> Self {
        Self::CommandExitNonZero {
            cmd: format!("{cmd:?}"),
            status,
            stdout: output
                .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
                .unwrap_or_default(),
            stderr: output
                .map(|output| String::from_utf8_lossy(&output.stderr).to_string())
                .unwrap_or_default(),
        }
    }

    pub fn cargo_metadata(manifest_dir: PathBuf, err: cargo_metadata::Error) -> Self {
        Self::CargoMetadata { manifest_dir, err }
    }

    pub fn cargo_root_package_not_found(manifest_dir: PathBuf) -> Self {
        Self::CargoRootPackageNotFound { manifest_dir }
    }

    pub fn env_var_error(var: String, var_error: std::env::VarError) -> Self {
        Self::Env { var, var_error }
    }
}

use crate::zkvm::ProofKind;
use std::{
    io,
    path::Path,
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

    #[error("Serialize {id} with `{lib}` failed: {err}")]
    Serialize {
        id: String,
        lib: String,
        #[source]
        err: anyhow::Error,
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

    #[error("Unsupported proof kind {unsupported:?}, expect one of {supported:?}")]
    UnsupportedProofKind {
        unsupported: ProofKind,
        supported: Vec<ProofKind>,
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

    pub fn file_not_found(id: impl AsRef<str>, path: impl AsRef<Path>) -> Self {
        let (id, path) = (id.as_ref(), path.as_ref().display());
        Self::io(
            format!("Failed to find {id} at {path}"),
            io::ErrorKind::NotFound.into(),
        )
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

    pub fn serialize(
        id: impl AsRef<str>,
        lib: impl AsRef<str>,
        err: impl Into<anyhow::Error>,
    ) -> Self {
        let id = id.as_ref().to_string();
        let lib = lib.as_ref().to_string();
        let err = err.into();
        Self::Serialize { id, lib, err }
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

    pub fn unsupported_proof_kind(
        unsupported: ProofKind,
        supported: impl IntoIterator<Item = ProofKind>,
    ) -> Self {
        Self::UnsupportedProofKind {
            unsupported,
            supported: supported.into_iter().collect(),
        }
    }
}

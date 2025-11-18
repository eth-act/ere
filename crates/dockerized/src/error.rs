use ere_server::client::{self, TwirpErrorResponse};
use std::{io, path::PathBuf};
use thiserror::Error;

impl From<client::Error> for Error {
    fn from(value: client::Error) -> Self {
        match value {
            client::Error::zkVM(err) => Self::zkVM(err),
            client::Error::ConnectionTimeout => Self::ConnectionTimeout,
            client::Error::Rpc(err) => Self::Rpc(err),
        }
    }
}

#[derive(Debug, Error)]
#[allow(non_camel_case_types)]
pub enum Error {
    #[error(
        "Guest directory must be in mounting directory, mounting_directory: {mounting_directory}, guest_directory: {guest_directory}"
    )]
    GuestNotInMountingDirecty {
        mounting_directory: PathBuf,
        guest_directory: PathBuf,
    },
    #[error("{context}: {source}")]
    Io {
        #[source]
        source: io::Error,
        context: String,
    },
    #[error("Failed to execute `docker image`: {0}")]
    DockerImageCmd(io::Error),
    #[error("Failed to execute `docker build`: {0}")]
    DockerBuildCmd(io::Error),
    #[error("Failed to execute `docker run`: {0}")]
    DockerRunCmd(io::Error),
    #[error("Failed to execute `docker container`: {0}")]
    DockerContainerCmd(io::Error),
    #[error("zkVM method error: {0}")]
    zkVM(String),
    #[error("Connection to zkVM server timeout after 5 minutes")]
    ConnectionTimeout,
    #[error("RPC to zkVM server error: {0}")]
    Rpc(TwirpErrorResponse),
}

impl Error {
    pub fn io(source: io::Error, context: impl ToString) -> Self {
        Self::Io {
            source,
            context: context.to_string(),
        }
    }
}

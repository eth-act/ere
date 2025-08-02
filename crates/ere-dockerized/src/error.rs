use std::{io, path::PathBuf};
use thiserror::Error;
use zkvm_interface::zkVMError;

impl From<DockerizedError> for zkVMError {
    fn from(value: DockerizedError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

impl From<CommonError> for zkVMError {
    fn from(value: CommonError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum DockerizedError {
    #[error(transparent)]
    Compile(#[from] CompileError),

    #[error(transparent)]
    Execute(#[from] ExecuteError),

    #[error(transparent)]
    Prove(#[from] ProveError),

    #[error(transparent)]
    Verify(#[from] VerifyError),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(
        "Guest directory must be in mounting directory, mounting_directory: {mounting_directory}, guest_directory: {guest_directory}"
    )]
    GuestNotInMountingDirecty {
        mounting_directory: PathBuf,
        guest_directory: PathBuf,
    },
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub enum CommonError {
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
    #[error("{context}: {source}")]
    Serialization {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
        context: String,
    },
}

impl CommonError {
    pub fn io(source: io::Error, context: impl ToString) -> Self {
        Self::Io {
            source,
            context: context.to_string(),
        }
    }

    pub fn serilization(
        source: impl Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
        context: impl ToString,
    ) -> Self {
        Self::Serialization {
            source: source.into(),
            context: context.to_string(),
        }
    }
}

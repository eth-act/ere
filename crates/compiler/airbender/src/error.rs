use airbender_build::BuildError;
use ere_util_compile::CommonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),
    #[error(transparent)]
    Build(#[from] BuildError),
}

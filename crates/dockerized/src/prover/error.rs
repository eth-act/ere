use core::time::Duration;

use ere_catalog::zkVMKind;
use ere_prover_core::CommonError;
use ere_server_client::client::{self, ParseError, TwirpErrorResponse};
use thiserror::Error;

use crate::util::docker::ContainerExitInfo;

impl From<client::Error> for Error {
    fn from(value: client::Error) -> Self {
        match value {
            client::Error::ParseUrl(err) => Self::ParseUrl(err),
            client::Error::zkVMProver(err) => Self::zkVMProver(err),
            client::Error::Rpc(err) => Self::Rpc(err),
        }
    }
}

#[derive(Debug, Error)]
#[allow(non_camel_case_types)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),
    #[error(transparent)]
    ParseUrl(#[from] ParseError),
    #[error(
        "Multiple CUDA architectures are not supported for {0:?}, CUDA_ARCHS set or detected: {1:?}"
    )]
    UnsupportedMultiCudaArchs(zkVMKind, Vec<u32>),
    #[error("zkVMProver method error: {0}")]
    zkVMProver(String),
    #[error("Connection to zkVMProver server timeout after 5 minutes")]
    ConnectionTimeout,
    #[error("RPC to zkVMProver server error: {0}")]
    Rpc(TwirpErrorResponse),
    #[error("Server container '{container_id}' exited during request: {exit_info}")]
    ContainerExited {
        container_id: String,
        exit_info: ContainerExitInfo,
    },
    #[error("Operation timed out after {timeout:?}")]
    Timeout { timeout: Duration },
}

use ere_server::client::{self, ParseError, TwirpErrorResponse};
use ere_zkvm_interface::CommonError;
use thiserror::Error;

impl From<client::Error> for Error {
    fn from(value: client::Error) -> Self {
        match value {
            client::Error::ParseUrl(err) => Self::ParseUrl(err),
            client::Error::zkVM(err) => Self::zkVM(err),
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
    #[error("zkVM method error: {0}")]
    zkVM(String),
    #[error("Connection to zkVM server timeout after 5 minutes")]
    ConnectionTimeout,
    #[error("RPC to zkVM server error: {0}")]
    Rpc(TwirpErrorResponse),
}
